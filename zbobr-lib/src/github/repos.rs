use std::path::PathBuf;

use crate::{Zbobr, ZbobrError};

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct RepoResponse {
    full_name: String,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct ContentsResponse {
    sha: Option<String>,
}

impl Zbobr {
    /// Check if a file exists in the domain repo.
    pub(crate) async fn repo_file_exists(&self, path: &str) -> Result<bool, ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let result = self
            .octocrab
            .get::<ContentsResponse, _, _>(
                format!("/repos/{owner}/{repo}/contents/{path}"),
                None::<&()>,
            )
            .await;
        Ok(result.is_ok())
    }

    /// Create or update a file in the domain repo via the Contents API.
    /// Content is provided as a plain string (will be base64-encoded).
    /// Skips if the file already exists (no overwrite).
    pub(crate) async fn create_repo_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let encoded = base64_encode(content);
        self.octocrab
            .put(
                format!("/repos/{owner}/{repo}/contents/{path}"),
                Some(&serde_json::json!({
                    "message": commit_message,
                    "content": encoded,
                })),
            )
            .await
            .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    /// Ensure the domain repo exists; create it if not.
    pub(crate) async fn ensure_domain_repo_exists(&self) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let exists = self
            .octocrab
            .get::<RepoResponse, _, _>(format!("/repos/{owner}/{repo}"), None::<&()>)
            .await
            .is_ok();

        if !exists {
            tracing::info!("Domain repo {owner}/{repo} does not exist, creating...");
            // Try creating as org repo first, fall back to user repo
            let result = self
                .octocrab
                .post(
                    format!("/orgs/{owner}/repos"),
                    Some(&serde_json::json!({
                        "name": repo,
                        "auto_init": true,
                    })),
                )
                .await;

            match result {
                Ok(_v) => {
                    let _: serde_json::Value = _v;
                    tracing::info!("Created org repo {owner}/{repo}");
                }
                Err(_) => {
                    // Fall back to user repo
                    self.octocrab
                        .post(
                            "/user/repos".to_string(),
                            Some(&serde_json::json!({
                                "name": repo,
                                "auto_init": true,
                            })),
                        )
                        .await
                        .map(|_: serde_json::Value| ())?;
                    tracing::info!("Created user repo {owner}/{repo}");
                }
            }
            // Wait for repo init
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        Ok(())
    }

    /// Ensure a fork exists under fork_owner for the given target repo.
    /// Returns the fork's "owner/repo" string.
    pub(crate) async fn ensure_fork(&self, target_repo: &str) -> Result<String, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let fork_repo = format!("{}/{}", self.config.fork_owner, repo_name);

        // Check if fork already exists
        let exists = self
            .octocrab
            .get::<RepoResponse, _, _>(format!("/repos/{fork_repo}"), None::<&()>)
            .await
            .is_ok();

        if !exists {
            let parts: Vec<&str> = target_repo.splitn(2, '/').collect();
            if parts.len() != 2 {
                return Err(ZbobrError::Config(format!(
                    "Invalid target repo: {target_repo}"
                )));
            }
            // Create fork under fork_owner (as org)
            self.octocrab
                .post(
                    format!("/repos/{}/{}/forks", parts[0], parts[1]),
                    Some(&serde_json::json!({ "organization": self.config.fork_owner })),
                )
                .await
                .map(|_: serde_json::Value| ())?;

            // Wait a moment for the fork to be ready
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        Ok(fork_repo)
    }

    /// Clone a repo into the workspace, set up fork remote and feature branch.
    /// Returns the local path.
    pub(crate) async fn clone_and_setup(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let issue_dir = self.config.workspace.join(format!("issue#{task_id}"));
        let work_dir = issue_dir.join(repo_name);

        tokio::fs::create_dir_all(&issue_dir).await?;

        // Clone if not already present
        if !work_dir.exists() {
            tracing::info!("Cloning {target_repo} into {}", work_dir.display());
            let status = tokio::process::Command::new("gh")
                .args(["repo", "clone", target_repo, work_dir.to_str().unwrap()])
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other(format!(
                    "Failed to clone {target_repo}"
                )));
            }
        }

        // Ensure fork exists
        let fork_repo = self.ensure_fork(target_repo).await?;

        // Add fork remote if not present
        let remote_check = tokio::process::Command::new("git")
            .args(["remote", "get-url", "fork"])
            .current_dir(&work_dir)
            .output()
            .await?;

        if !remote_check.status.success() {
            tracing::info!("Adding fork remote for {fork_repo}");
            let status = tokio::process::Command::new("git")
                .args([
                    "remote",
                    "add",
                    "fork",
                    &format!("https://github.com/{fork_repo}.git"),
                ])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other("Failed to add fork remote".into()));
            }
        }

        // Create/checkout feature branch
        let branch_name = format!("fix{task_id}/implementation");
        let branch_exists = tokio::process::Command::new("git")
            .args(["rev-parse", "--verify", &branch_name])
            .current_dir(&work_dir)
            .output()
            .await?;

        if branch_exists.status.success() {
            let _ = tokio::process::Command::new("git")
                .args(["checkout", &branch_name])
                .current_dir(&work_dir)
                .status()
                .await?;
        } else {
            let status = tokio::process::Command::new("git")
                .args(["checkout", "-b", &branch_name])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other(format!(
                    "Failed to create branch {branch_name}"
                )));
            }
        }

        Ok(work_dir)
    }

    /// Clone a repo for read-only investigation (no fork, no branch).
    pub(crate) async fn clone_readonly(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let issue_dir = self.config.workspace.join(format!("issue#{task_id}"));
        let work_dir = issue_dir.join(repo_name);

        tokio::fs::create_dir_all(&issue_dir).await?;

        if !work_dir.exists() {
            tracing::info!("Cloning {target_repo} (read-only) into {}", work_dir.display());
            let status = tokio::process::Command::new("gh")
                .args(["repo", "clone", target_repo, work_dir.to_str().unwrap()])
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other(format!(
                    "Failed to clone {target_repo}"
                )));
            }
        }

        Ok(work_dir)
    }

    /// Push the current branch to the fork remote and create a PR.
    pub(crate) async fn push_and_create_pr(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let work_dir = self
            .config
            .workspace
            .join(format!("issue#{task_id}"))
            .join(repo_name);

        if !work_dir.exists() {
            return Err(ZbobrError::Other(format!(
                "Work directory does not exist: {}",
                work_dir.display()
            )));
        }

        let branch_name = format!("fix{task_id}/implementation");

        // Push to fork
        tracing::info!("Pushing {branch_name} to fork");
        let status = tokio::process::Command::new("git")
            .args(["push", "fork", "HEAD"])
            .current_dir(&work_dir)
            .status()
            .await?;
        if !status.success() {
            return Err(ZbobrError::Other("Failed to push to fork".into()));
        }

        // Create PR using gh CLI
        let task = self.get_issue(task_id).await?;
        let pr_title = format!("Fix #{task_id}: {}", task.title);
        let pr_body = format!(
            "Resolves #{task_id}\n\nImplementation for: {}",
            task.title
        );

        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "create",
                "--repo",
                target_repo,
                "--head",
                &format!("{}:{branch_name}", self.config.fork_owner),
                "--title",
                &pr_title,
                "--body",
                &pr_body,
            ])
            .current_dir(&work_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ZbobrError::Other(format!("Failed to create PR: {stderr}")));
        }

        let pr_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(pr_url)
    }
}

/// Simple base64 encoder (standard alphabet, with padding).
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

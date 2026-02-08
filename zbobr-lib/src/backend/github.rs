use super::Backend;
use crate::{Stage, Task, ZbobrConfig, ZbobrError};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

pub struct GitHubBackend {
    config: Arc<ZbobrConfig>,
    octocrab: octocrab::Octocrab,
}

impl GitHubBackend {
    pub fn new(config: Arc<ZbobrConfig>, octocrab: octocrab::Octocrab) -> Self {
        Self { config, octocrab }
    }

    fn parse_repo(&self) -> Result<(&str, &str), ZbobrError> {
        self.config.parse_repo()
    }

    async fn find_milestone_number(&self, title: &str) -> Result<Option<u64>, ZbobrError> {
        let milestones = self.list_milestones().await?;
        Ok(milestones
            .into_iter()
            .find(|(_, t)| t == title)
            .map(|(n, _)| n))
    }

    async fn ensure_fork(&self, target_repo: &str) -> Result<String, ZbobrError> {
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
}

#[derive(Debug, serde::Deserialize)]
struct IssueResponse {
    number: u64,
    title: String,
    body: Option<String>,
    #[allow(dead_code)]
    state: String,
    milestone: Option<IssueMilestone>,
    labels: Vec<IssueLabel>,
}

#[derive(Debug, serde::Deserialize)]
struct IssueMilestone {
    title: String,
}

#[derive(Debug, serde::Deserialize)]
struct IssueLabel {
    name: String,
}

#[derive(Debug, serde::Deserialize)]
struct CommentResponse {
    user: Option<CommentUser>,
    body: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct CommentUser {
    login: String,
}

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

#[derive(Debug, serde::Deserialize)]
struct MilestoneResponse {
    number: u64,
    title: String,
}

#[async_trait]
impl Backend for GitHubBackend {
    async fn get_issue(&self, issue_number: u64) -> Result<Task, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let issue: IssueResponse = self
            .octocrab
            .get(
                format!("/repos/{owner}/{repo}/issues/{issue_number}"),
                None::<&()>,
            )
            .await?;

        let stage = match issue.milestone.as_ref().map(|m| m.title.as_str()) {
            Some("PLANNING") => Stage::Planning,
            Some("PENDING") => Stage::Pending,
            Some("PLANNING_READY") => Stage::PlanningReady,
            Some("WORKING_READY") => Stage::WorkingReady,
            Some("WORKING") => Stage::Working,
            _ => Stage::Planning, // default
        };

        let model = issue
            .labels
            .iter()
            .find_map(|l| l.name.strip_prefix("copilot:").map(String::from));

        let done = issue.labels.iter().any(|l| l.name == "done");

        Ok(Task {
            id: issue.number,
            title: issue.title,
            description: issue.body.unwrap_or_default(),
            stage,
            model,
            done,
        })
    }

    async fn get_issue_comments(&self, issue_number: u64) -> Result<Vec<String>, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let comments: Vec<CommentResponse> = self
            .octocrab
            .get(
                format!("/repos/{owner}/{repo}/issues/{issue_number}/comments"),
                None::<&()>,
            )
            .await?;

        Ok(comments
            .into_iter()
            .map(|c| {
                let user = c.user.map(|u| u.login).unwrap_or_else(|| "unknown".into());
                let body = c.body.unwrap_or_default();
                format!("{user}: {body}")
            })
            .collect())
    }

    async fn post_issue_comment(&self, issue_number: u64, body: &str) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        self.octocrab
            .issues(owner, repo)
            .create_comment(issue_number, body)
            .await?;
        Ok(())
    }

    async fn set_issue_milestone(
        &self,
        issue_number: u64,
        milestone_title: &str,
    ) -> Result<(), ZbobrError> {
        let milestone_number = self
            .find_milestone_number(milestone_title)
            .await?
            .ok_or_else(|| {
                ZbobrError::GitHub(format!("Milestone '{milestone_title}' not found"))
            })?;

        let (owner, repo) = self.parse_repo()?;
        self.octocrab
            .patch(
                format!("/repos/{owner}/{repo}/issues/{issue_number}"),
                Some(&serde_json::json!({ "milestone": milestone_number })),
            )
            .await
            .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    async fn add_issue_label(&self, issue_number: u64, label: &str) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        self.octocrab
            .issues(owner, repo)
            .add_labels(issue_number, &[label.to_string()])
            .await?;
        Ok(())
    }

    async fn remove_issue_label(&self, issue_number: u64, label: &str) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        // Removing a label that doesn't exist returns 404, which we ignore.
        let _ = self
            .octocrab
            .issues(owner, repo)
            .remove_label(issue_number, label)
            .await;
        Ok(())
    }

    async fn update_issue_body(&self, issue_number: u64, body: &str) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        self.octocrab
            .patch(
                format!("/repos/{owner}/{repo}/issues/{issue_number}"),
                Some(&serde_json::json!({ "body": body })),
            )
            .await
            .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    async fn list_issues_by_milestone(
        &self,
        milestone_title: &str,
    ) -> Result<Vec<Task>, ZbobrError> {
        let milestone_number = match self.find_milestone_number(milestone_title).await? {
            Some(n) => n,
            None => return Ok(vec![]),
        };

        let (owner, repo) = self.parse_repo()?;
        let issues: Vec<IssueResponse> = self
            .octocrab
            .get(
                format!("/repos/{owner}/{repo}/issues"),
                Some(&[
                    ("milestone", milestone_number.to_string().as_str()),
                    ("state", "open"),
                ]),
            )
            .await?;

        let mut tasks = Vec::new();
        for issue in issues {
            let stage = match issue.milestone.as_ref().map(|m| m.title.as_str()) {
                Some("PLANNING") => Stage::Planning,
                Some("PENDING") => Stage::Pending,
                Some("PLANNING_READY") => Stage::PlanningReady,
                Some("WORKING_READY") => Stage::WorkingReady,
                Some("WORKING") => Stage::Working,
                _ => Stage::Planning,
            };
            let model = issue
                .labels
                .iter()
                .find_map(|l| l.name.strip_prefix("copilot:").map(String::from));
            let done = issue.labels.iter().any(|l| l.name == "done");
            tasks.push(Task {
                id: issue.number,
                title: issue.title,
                description: issue.body.unwrap_or_default(),
                stage,
                model,
                done,
            });
        }
        Ok(tasks)
    }

    async fn is_issue_closed(&self, issue_number: u64) -> Result<bool, ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let issue: IssueResponse = self
            .octocrab
            .get(
                format!("/repos/{owner}/{repo}/issues/{issue_number}"),
                None::<&()>,
            )
            .await?;
        Ok(issue.state == "closed")
    }

    async fn repo_file_exists(&self, path: &str) -> Result<bool, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let result = self
            .octocrab
            .get::<ContentsResponse, _, _>(
                format!("/repos/{owner}/{repo}/contents/{path}"),
                None::<&()>,
            )
            .await;
        Ok(result.is_ok())
    }

    async fn create_repo_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
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

    async fn ensure_domain_repo_exists(&self) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
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
                        "private": true,
                        "auto_init": true,
                    })),
                )
                .await;

            match result {
                Ok(_v) => {
                    let _: serde_json::Value = _v;
                    tracing::info!("Created private org repo {owner}/{repo}");
                }
                Err(_) => {
                    // Fall back to user repo
                    self.octocrab
                        .post(
                            "/user/repos".to_string(),
                            Some(&serde_json::json!({
                                "name": repo,
                                "private": true,
                                "auto_init": true,
                            })),
                        )
                        .await
                        .map(|_: serde_json::Value| ())?;
                    tracing::info!("Created private user repo {owner}/{repo}");
                }
            }
            // Wait for repo init
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        Ok(())
    }

    async fn clone_and_setup(
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
                return Err(ZbobrError::Other(format!("Failed to clone {target_repo}")));
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

    async fn clone_readonly(&self, target_repo: &str, task_id: u64) -> Result<PathBuf, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let issue_dir = self.config.workspace.join(format!("issue#{task_id}"));
        let work_dir = issue_dir.join(repo_name);

        tokio::fs::create_dir_all(&issue_dir).await?;

        if !work_dir.exists() {
            tracing::info!(
                "Cloning {target_repo} (read-only) into {}",
                work_dir.display()
            );
            let status = tokio::process::Command::new("gh")
                .args(["repo", "clone", target_repo, work_dir.to_str().unwrap()])
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other(format!("Failed to clone {target_repo}")));
            }
        }

        Ok(work_dir)
    }

    async fn push_and_create_pr(
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
        let pr_body = format!("Resolves #{task_id}\n\nImplementation for: {}", task.title);

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

    async fn list_milestones(&self) -> Result<Vec<(u64, String)>, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let milestones: Vec<MilestoneResponse> = self
            .octocrab
            .get(format!("/repos/{owner}/{repo}/milestones"), None::<&()>)
            .await?;
        Ok(milestones
            .into_iter()
            .map(|m| (m.number, m.title))
            .collect())
    }

    async fn create_milestone(&self, title: &str, description: &str) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        self.octocrab
            .post(
                format!("/repos/{owner}/{repo}/milestones"),
                Some(&serde_json::json!({
                    "title": title,
                    "description": description,
                    "state": "open"
                })),
            )
            .await
            .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    async fn delete_milestone(&self, number: u64) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let _response = self
            .octocrab
            ._delete(
                format!("/repos/{owner}/{repo}/milestones/{number}"),
                None::<&()>,
            )
            .await?;
        Ok(())
    }

    async fn list_labels(&self) -> Result<Vec<String>, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let labels: Vec<octocrab::models::Label> = self
            .octocrab
            .issues(owner, repo)
            .list_labels_for_repo()
            .per_page(100)
            .send()
            .await?
            .items;
        Ok(labels.into_iter().map(|l| l.name).collect())
    }

    async fn create_label(
        &self,
        name: &str,
        color: &str,
        description: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        self.octocrab
            .issues(owner, repo)
            .create_label(name, color, description)
            .await?;
        Ok(())
    }
}

/// Simple base64 encoder (standard alphabet, with padding).
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
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

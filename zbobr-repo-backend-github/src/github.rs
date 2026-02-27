use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use zbobr_dispatcher::backend::RepoBackend;

use crate::config::ZbobrRepoBackendGithubConfig;

/// Convert an octocrab error into an anyhow::Error with detailed information.
fn octocrab_to_anyhow(e: octocrab::Error) -> anyhow::Error {
    match e {
        octocrab::Error::GitHub { source, .. } => {
            anyhow::anyhow!(
                "GitHub API error: {} (status: {}) -- details: {:?}",
                source.message,
                source.status_code,
                source
            )
        }
        other => anyhow::anyhow!("GitHub API error: {:?}", other),
    }
}

fn is_transient_octocrab_error(error: &octocrab::Error) -> bool {
    match error {
        octocrab::Error::GitHub { source, .. } => source.status_code.is_server_error(),
        _ => true,
    }
}

/// Retry a GitHub API operation up to 3 times on transient errors.
async fn retry_github<T, F, Fut>(op_name: &str, mut f: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, octocrab::Error>>,
{
    let mut attempt = 0u64;
    loop {
        attempt += 1;
        match f().await {
            Ok(value) => return Ok(value),
            Err(e) => {
                if attempt < 3 && is_transient_octocrab_error(&e) {
                    tracing::warn!(
                        "Transient GitHub error during {op_name} (attempt {attempt}/3): {e}"
                    );
                    tokio::time::sleep(Duration::from_millis(250 * attempt)).await;
                    continue;
                }
                return Err(octocrab_to_anyhow(e));
            }
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct RepoResponse {
    full_name: String,
}

#[derive(Debug)]
struct GitHubRepo {
    full_name: String,
}

impl GitHubRepo {
    /// Returns the owner part of the repository (before the '/').
    fn owner(&self) -> &str {
        self.full_name.split('/').next().unwrap_or("")
    }

    /// Returns the name part of the repository (after the '/').
    fn name(&self) -> &str {
        self.full_name.split('/').nth(1).unwrap_or("")
    }
}

fn parse_github_repo(repo_ref: &str) -> anyhow::Result<GitHubRepo> {
    // Standardize: remove trailing .git and /
    let repo_ref = repo_ref.trim_end_matches(".git").trim_end_matches('/');

    let full_name = if repo_ref.contains("://") {
        // extract owner/repo from URL
        let parts: Vec<&str> = repo_ref.split('/').collect();
        if parts.len() < 2 {
            anyhow::bail!("Invalid GitHub URL: {}", repo_ref);
        }
        format!("{}/{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else if repo_ref.contains(':') {
        // git@github.com:owner/repo
        repo_ref.rsplit(':').next().unwrap_or("").to_string()
    } else {
        repo_ref.to_string()
    };

    let parts: Vec<&str> = full_name.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "Invalid GitHub repository format: {}. Expected 'owner/repo' or a GitHub URL.",
            repo_ref
        );
    }

    Ok(GitHubRepo { full_name })
}

// ============================================================================
// GitHubRepoBackend
// ============================================================================

pub struct GitHubRepoBackend {
    backend_config: ZbobrRepoBackendGithubConfig,
    octocrab: octocrab::Octocrab,
    git_user_name: String,
    git_user_email: String,
}

impl GitHubRepoBackend {
    pub fn new(
        toml: Option<crate::config::ZbobrRepoBackendGithubToml>,
        args: crate::config::ZbobrRepoBackendGithubArgs,
        git_user_name: String,
        git_user_email: String,
    ) -> anyhow::Result<Self> {
        let backend_config = ZbobrRepoBackendGithubConfig::build(toml, args);
        backend_config.validate()?;
        let octocrab = octocrab::Octocrab::builder()
            .personal_token(backend_config.token.clone())
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build octocrab client: {e}"))?;
        Ok(Self {
            backend_config,
            octocrab,
            git_user_name,
            git_user_email,
        })
    }

    async fn ensure_fork(
        &self,
        target_repo: &str,
        destination_branch: &str,
    ) -> anyhow::Result<String> {
        let repo = parse_github_repo(target_repo)?;
        let fork_repo = format!("{}/{}", self.backend_config.fork_owner, repo.name());

        // Check if fork already exists
        let exists = retry_github("check fork exists", || {
            self.octocrab
                .get::<RepoResponse, _, _>(format!("/repos/{fork_repo}"), None::<&()>)
        })
        .await
        .is_ok();

        if !exists {
            let fork_owner = &self.backend_config.fork_owner;
            let endpoint = format!("/repos/{}/forks", repo.full_name);
            let payload = serde_json::json!({ "organization": fork_owner });

            tracing::info!(
                "Creating fork of {target_repo} under organization '{fork_owner}' using endpoint {endpoint}"
            );
            tracing::debug!("Fork creation payload: {payload}");

            retry_github("create fork", || {
                self.octocrab.post(&endpoint, Some(&payload))
            })
            .await
            .map_err(|e| {
                let error_details = format!("{:?}", e);
                tracing::error!(
                    "Failed to create fork: target_repo={}, fork_owner={}, endpoint={}, error={:?}",
                    target_repo,
                    fork_owner,
                    endpoint,
                    e
                );
                anyhow::anyhow!(
                    "Failed to create fork of {target_repo} under '{fork_owner}': \
                         check if fork_owner is an organization you have access to, \
                         and that your GitHub token has 'repo' and 'admin:org_hook' scopes. \
                         Endpoint: {endpoint}. Error: {e}\n\
                         Debug: {error_details}",
                )
            })
            .map(|_: serde_json::Value| ())?;

            // Wait a moment for the fork to be ready
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        } else {
            // Fork exists — sync it with the destination branch on upstream.
            // The destination branch (e.g. main) must exist on upstream; if it
            // somehow doesn't, that is an error we should surface.
            let endpoint = format!("/repos/{}/merge-upstream", fork_repo);
            let body = serde_json::json!({ "branch": destination_branch });

            tracing::info!(
                "Syncing fork {fork_repo} with upstream {}/{}",
                repo.full_name,
                destination_branch
            );

            self.octocrab
                .post::<serde_json::Value, serde_json::Value>(endpoint, Some(&body))
                .await
                .with_context(|| {
                    format!(
                        "Failed to sync fork {fork_repo} with upstream {}/{destination_branch}",
                        repo.full_name
                    )
                })?;

            tracing::info!("Successfully synced fork {fork_repo} with upstream destination branch '{destination_branch}'");
        }

        Ok(fork_repo)
    }

    /// Query GitHub for an existing open PR matching `head` → `base`.
    /// Returns the `html_url` of the first matching PR.
    async fn find_existing_pr(
        &self,
        full_repo: &str,
        head: &str,
        base: &str,
    ) -> anyhow::Result<String> {
        #[derive(serde::Deserialize)]
        struct PrListItem {
            html_url: String,
        }

        // GitHub's list-PRs API requires "owner:branch" format for the head
        // filter; a bare branch name is silently ignored and all open PRs are
        // returned, causing the wrong PR to be selected.
        let owner = full_repo.split('/').next().unwrap_or(full_repo);
        let head_filter = format!("{owner}:{head}");

        let endpoint = format!("/repos/{full_repo}/pulls");
        let params = serde_json::json!({
            "head": head_filter,
            "base": base,
            "state": "open",
        });

        let prs: Vec<PrListItem> = self
            .octocrab
            .get(&endpoint, Some(&params))
            .await
            .map_err(octocrab_to_anyhow)?;

        prs.into_iter()
            .next()
            .map(|pr| pr.html_url)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No existing open PR found for head '{}' -> base '{}' in {}",
                    head,
                    base,
                    full_repo
                )
            })
    }
}

#[async_trait]
impl RepoBackend for GitHubRepoBackend {
    async fn clone_and_setup(
        &self,
        target_repo: &str,
        work_branch: &str,
        destination_branch: &str,
        workspace_path: &std::path::Path,
    ) -> anyhow::Result<PathBuf> {
        let repo = parse_github_repo(target_repo)?;
        let repo_name = repo.name();

        let work_dir = workspace_path.join(repo_name);

        tokio::fs::create_dir_all(workspace_path).await?;

        // Clone or update the destination repo
        if !work_dir.exists() {
            // Fresh clone: always clone the destination branch (not the work branch,
            // which may not exist on origin yet).
            tracing::info!(
                "Cloning {target_repo} (branch '{destination_branch}') into {}",
                work_dir.display()
            );
            let status = tokio::process::Command::new("gh")
                .args([
                    "repo",
                    "clone",
                    target_repo,
                    work_dir.to_str().unwrap(),
                    "--",
                    "--branch",
                    destination_branch,
                    "--single-branch",
                    "--depth",
                    "1",
                ])
                .env("GH_TOKEN", &self.backend_config.token)
                .env("GITHUB_TOKEN", &self.backend_config.token)
                .status()
                .await?;
            if !status.success() {
                anyhow::bail!(
                    "Failed to clone {target_repo} at destination branch '{destination_branch}'"
                );
            }
        } else {
            // Workspace exists: fetch the destination branch from origin and
            // force-reset the local branch to match it exactly.
            tracing::info!("Updating {target_repo} in {}", work_dir.display());
            let fetch_output = tokio::process::Command::new("git")
                .args(["fetch", "--depth", "1", "origin", destination_branch])
                .current_dir(&work_dir)
                .output()
                .await?;
            if !fetch_output.status.success() {
                let stderr = String::from_utf8_lossy(&fetch_output.stderr);
                anyhow::bail!(
                    "Failed to fetch destination branch '{destination_branch}' from \
                     {target_repo}: {stderr}"
                );
            }

            // Force-reset the local destination branch so it is identical to origin.
            // Use `git branch -f` so we don't have to switch branches first.
            let reset_output = tokio::process::Command::new("git")
                .args([
                    "branch",
                    "-f",
                    destination_branch,
                    &format!("origin/{destination_branch}"),
                ])
                .current_dir(&work_dir)
                .output()
                .await?;
            if !reset_output.status.success() {
                let stderr = String::from_utf8_lossy(&reset_output.stderr);
                anyhow::bail!(
                    "Failed to reset local branch '{destination_branch}' to \
                     origin/{destination_branch}: {stderr}"
                );
            }
            tracing::info!(
                "Reset local '{destination_branch}' to match origin/{destination_branch}"
            );
        }

        // Checkout the work branch
        tracing::info!("Checking out branch {work_branch}");
        let checkout_status = tokio::process::Command::new("git")
            .args(["checkout", work_branch])
            .current_dir(&work_dir)
            .status()
            .await?;
        if !checkout_status.success() {
            let checkout_remote_status = tokio::process::Command::new("git")
                .args(["checkout", "-b", work_branch, &format!("origin/{work_branch}")])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !checkout_remote_status.success() {
                // Branch doesn't exist on remote — create it fresh from the destination branch.
                tracing::info!("Creating new local branch {work_branch} from {destination_branch}");
                let create_status = tokio::process::Command::new("git")
                    .args(["checkout", "-b", work_branch])
                    .current_dir(&work_dir)
                    .status()
                    .await?;
                if !create_status.success() {
                    anyhow::bail!("Failed to checkout branch {work_branch}");
                }
            }
        }

        // In same-org mode (fork_owner == target repo owner), work directly on
        // the repo without forking.  In cross-org mode, ensure the fork exists,
        // sync the destination branch in the fork, and add a "fork" remote.
        let same_org = repo.owner().eq_ignore_ascii_case(&self.backend_config.fork_owner);
        if same_org {
            tracing::info!("Same-org mode: skipping fork setup for {target_repo}");
        } else {
            let fork_repo = self.ensure_fork(target_repo, destination_branch).await?;

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
                    anyhow::bail!("Failed to add fork remote");
                }
            }
        }

        Ok(work_dir)
    }

    async fn clone_readonly(
        &self,
        target_repo: &str,
        branch: &str,
        workspace_path: &std::path::Path,
    ) -> anyhow::Result<PathBuf> {
        let repo = parse_github_repo(target_repo)?;
        let repo_name = repo.name();

        let work_dir = workspace_path.join(repo_name);

        tokio::fs::create_dir_all(workspace_path).await?;

        if !work_dir.exists() {
            tracing::info!(
                "Cloning {target_repo} (read-only) into {},",
                work_dir.display()
            );
            let status = tokio::process::Command::new("gh")
                .args([
                    "repo",
                    "clone",
                    target_repo,
                    work_dir.to_str().unwrap(),
                    "--",
                    "--branch",
                    branch,
                    "--single-branch",
                    "--depth",
                    "1",
                ])
                .env("GH_TOKEN", &self.backend_config.token)
                .env("GITHUB_TOKEN", &self.backend_config.token)
                .status()
                .await?;
            if !status.success() {
                anyhow::bail!("Failed to clone {target_repo}");
            }
        } else {
            tracing::info!(
                "Updating {target_repo} (read-only) in {}",
                work_dir.display()
            );

            let fetch_status = tokio::process::Command::new("git")
                .args(["fetch", "--depth", "1", "origin", branch])
                .current_dir(&work_dir)
                .status()
                .await?;

            if !fetch_status.success() {
                tracing::warn!(
                    "Failed to fetch latest changes for {target_repo}, using existing state"
                );
            }
        }

        // Checkout the requested branch
        tracing::info!("Checking out branch {branch} (read-only)");
        let checkout_status = tokio::process::Command::new("git")
            .args(["checkout", branch])
            .current_dir(&work_dir)
            .status()
            .await?;
        if !checkout_status.success() {
            let checkout_remote_status = tokio::process::Command::new("git")
                .args(["checkout", "-b", branch, &format!("origin/{branch}")])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !checkout_remote_status.success() {
                anyhow::bail!("Failed to checkout branch {branch}");
            }
        }

        Ok(work_dir)
    }

    async fn ensure_branch_and_pr(
        &self,
        target_repo: &str,
        workspace_path: &std::path::Path,
        work_branch: &str,
        destination_branch: &str,
        pr_title: &str,
    ) -> anyhow::Result<String> {
        let repo = parse_github_repo(target_repo)?;
        let work_dir = workspace_path.join(repo.name());

        if !work_dir.exists() {
            anyhow::bail!("Work directory does not exist: {}", work_dir.display());
        }

        // Check if the branch has any commits ahead of the destination branch.
        // If not, create a placeholder commit so GitHub PR API won't reject it.
        let log_out = tokio::process::Command::new("git")
            .args([
                "log",
                &format!("origin/{}..HEAD", destination_branch),
                "--oneline",
            ])
            .current_dir(&work_dir)
            .output()
            .await
            .context("Failed to check commits ahead of destination branch")?;

        let has_commits_ahead = log_out.status.success()
            && !String::from_utf8_lossy(&log_out.stdout).trim().is_empty();

        if !has_commits_ahead {
            tracing::info!(
                "No commits ahead of origin/{destination_branch} — creating placeholder commit"
            );
            zbobr_dispatcher::backend::configure_git_user(
                &work_dir,
                &self.git_user_name,
                &self.git_user_email,
            )
            .await
            .context("Failed to configure git user for placeholder commit")?;
            zbobr_dispatcher::backend::create_placeholder_commit(&work_dir, work_branch)
                .await
                .context("Failed to create placeholder commit")?;
        }

        // In same-org mode there is no "fork" remote — push directly to origin
        // and use a simple branch name for the PR head.  In cross-org mode push
        // to the fork remote and prefix the head with the fork owner.
        let has_fork_remote = tokio::process::Command::new("git")
            .args(["remote", "get-url", "fork"])
            .current_dir(&work_dir)
            .output()
            .await?
            .status
            .success();

        // In cross-org mode push to the fork remote and create the PR inside
        // the fork.  The user is responsible for retargeting the PR to the
        // upstream repo.  In same-org mode push directly to origin and create
        // the PR there.
        let (push_remote, pr_repo) = if has_fork_remote {
            (
                "fork",
                format!("{}/{}", self.backend_config.fork_owner, repo.name()),
            )
        } else {
            ("origin", repo.full_name.clone())
        };

        tracing::info!("Pushing {work_branch} to {push_remote}");
        let status = tokio::process::Command::new("git")
            .args(["push", "--force", push_remote, "HEAD"])
            .current_dir(&work_dir)
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("Failed to push to {push_remote}");
        }

        // Try to create the PR.  If GitHub returns 422 (PR already exists for this branch),
        // query the open PRs to find and return the existing PR URL.
        let pr_payload = serde_json::json!({
            "title": pr_title,
            "head": work_branch,
            "base": destination_branch,
            "body": "",
        });

        #[derive(serde::Deserialize)]
        struct PrResponse {
            html_url: String,
        }

        let pr_endpoint = format!("/repos/{pr_repo}/pulls");

        let create_result: Result<PrResponse, octocrab::Error> = self
            .octocrab
            .post(pr_endpoint.clone(), Some(&pr_payload))
            .await;

        match create_result {
            Ok(pr) => Ok(pr.html_url),
            Err(octocrab::Error::GitHub { ref source, .. }) if source.status_code.as_u16() == 422 => {
                tracing::info!(
                    "PR already exists for {work_branch}, looking up existing PR"
                );
                self.find_existing_pr(&pr_repo, work_branch, destination_branch)
                    .await
            }
            Err(e) => Err(octocrab_to_anyhow(e)),
        }
    }

    async fn push_branch(
        &self,
        target_repo: &str,
        workspace_path: &std::path::Path,
        work_branch: &str,
    ) -> anyhow::Result<()> {
        let repo = parse_github_repo(target_repo)?;
        let work_dir = workspace_path.join(repo.name());

        if !work_dir.exists() {
            anyhow::bail!("Work directory does not exist: {}", work_dir.display());
        }

        let has_fork_remote = tokio::process::Command::new("git")
            .args(["remote", "get-url", "fork"])
            .current_dir(&work_dir)
            .output()
            .await?
            .status
            .success();

        let push_remote = if has_fork_remote { "fork" } else { "origin" };

        tracing::info!("Pushing {work_branch} to {push_remote}");
        let status = tokio::process::Command::new("git")
            .args(["push", "--force", push_remote, "HEAD"])
            .current_dir(&work_dir)
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("Failed to push to {push_remote}");
        }

        Ok(())
    }

    async fn create_pr_in_fork(
        &self,
        repo_name: &str,
        work_branch: &str,
        destination_branch: &str,
        pr_title: &str,
        pr_body: &str,
    ) -> anyhow::Result<String> {
        let fork_repo = format!("{}/{}", self.backend_config.fork_owner, repo_name);

        tracing::info!(
            "Creating PR in fork {} from {} to {}",
            fork_repo,
            work_branch,
            destination_branch
        );

        let pr_payload = serde_json::json!({
            "title": pr_title,
            "head": work_branch,
            "base": destination_branch,
            "body": pr_body,
        });

        let pr_endpoint = format!("/repos/{fork_repo}/pulls");

        #[derive(serde::Deserialize)]
        struct PrResponse {
            html_url: String,
        }

        let response: PrResponse = retry_github("create PR in fork", || {
            self.octocrab.post(pr_endpoint.clone(), Some(&pr_payload))
        })
        .await?;

        Ok(response.html_url)
    }

    async fn setup_fork_remote_and_push(
        &self,
        work_dir: &std::path::Path,
        target_repo: &str,
        work_branch: &str,
    ) -> anyhow::Result<()> {
        let repo = parse_github_repo(target_repo)?;
        let fork_repo = format!("{}/{}", self.backend_config.fork_owner, repo.name());
        let fork_url = format!("https://github.com/{fork_repo}.git");

        // Remove old "fork" remote (ignore error if it doesn't exist)
        let _ = tokio::process::Command::new("git")
            .args(["remote", "remove", "fork"])
            .current_dir(work_dir)
            .status()
            .await;

        // Remove origin remote and replace it with fork remote URL
        tracing::info!("Replacing origin remote with fork: {}", fork_url);
        let remove_origin = tokio::process::Command::new("git")
            .args(["remote", "remove", "origin"])
            .current_dir(work_dir)
            .status()
            .await?;

        if !remove_origin.success() {
            anyhow::bail!("Failed to remove origin remote");
        }

        let add_origin = tokio::process::Command::new("git")
            .args(["remote", "add", "origin", &fork_url])
            .current_dir(work_dir)
            .status()
            .await?;

        if !add_origin.success() {
            anyhow::bail!("Failed to add fork as origin remote");
        }

        // Push the work branch to the forked repository
        tracing::info!("Pushing work branch '{}' to fork", work_branch);
        let push_status = tokio::process::Command::new("git")
            .args(["push", "-u", "origin", work_branch])
            .current_dir(work_dir)
            .status()
            .await?;

        if !push_status.success() {
            anyhow::bail!("Failed to push work branch '{}' to fork", work_branch);
        }

        Ok(())
    }

    async fn parse_pr_to_repo_branch(&self, pr_ref: &str) -> anyhow::Result<(String, String)> {
        fn parse_ref(pr_ref: &str) -> anyhow::Result<(String, String, u64)> {
            if pr_ref.starts_with("https://github.com/") {
                let parts: Vec<&str> = pr_ref
                    .trim_start_matches("https://github.com/")
                    .split('/')
                    .collect();
                if parts.len() >= 4 && parts[2] == "pull" {
                    let pr_num = parts[3]
                        .parse::<u64>()
                        .map_err(|_| anyhow::anyhow!("Invalid PR number in URL: {pr_ref}"))?;
                    return Ok((parts[0].to_string(), parts[1].to_string(), pr_num));
                }
                return Err(anyhow::anyhow!("Invalid PR URL format: {pr_ref}"));
            }
            if pr_ref.contains('#') {
                let parts: Vec<&str> = pr_ref.split('#').collect();
                if parts.len() == 2 {
                    let repo_parts: Vec<&str> = parts[0].split('/').collect();
                    if repo_parts.len() == 2 {
                        let pr_num = parts[1]
                            .parse::<u64>()
                            .map_err(|_| anyhow::anyhow!("Invalid PR number: {}", parts[1]))?;
                        return Ok((
                            repo_parts[0].to_string(),
                            repo_parts[1].to_string(),
                            pr_num,
                        ));
                    }
                    return Err(anyhow::anyhow!("Invalid repo format in PR reference: {pr_ref}"));
                }
                return Err(anyhow::anyhow!("Invalid PR reference format: {pr_ref}"));
            }
            Err(anyhow::anyhow!(
                "PR reference must be a URL or owner/repo#number format: {pr_ref}"
            ))
        }

        let (owner, repo, pr_number) = parse_ref(pr_ref)?;

        #[derive(serde::Deserialize)]
        struct PrHead {
            #[serde(rename = "ref")]
            ref_name: String,
        }
        #[derive(serde::Deserialize)]
        struct PrView {
            head: PrHead,
        }

        let pr_endpoint = format!("/repos/{owner}/{repo}/pulls/{pr_number}");
        let pr: PrView = self
            .octocrab
            .get(pr_endpoint, None::<&()>)
            .await
            .map_err(|e| anyhow::anyhow!("GitHub API error: {}", e))?;

        let branch = pr.head.ref_name;
        let repo_full = format!("{owner}/{repo}");

        Ok((repo_full, branch))
    }

    async fn validate_connectivity(&self) -> anyhow::Result<()> {
        let fork_owner = &self.backend_config.fork_owner;
        let fork_owner_exists = retry_github("check fork owner", || {
            self.octocrab
                .get::<serde_json::Value, _, _>(format!("/users/{fork_owner}"), None::<&()>)
        })
        .await
        .is_ok();
        if !fork_owner_exists {
            anyhow::bail!(
                "fork_owner '{fork_owner}' does not exist on GitHub as a user or organization.\n  \
                 Check your fork_owner setting and ensure the account exists."
            );
        }

        Ok(())
    }

    fn debug_state(&self) -> String {
        format!(
            "GitHubRepoBackend(fork_owner={})",
            self.backend_config.fork_owner
        )
    }
}

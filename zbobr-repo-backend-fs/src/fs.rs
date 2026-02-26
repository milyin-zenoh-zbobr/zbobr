use std::path::{Path, PathBuf};

use anyhow::Context;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;
use zbobr_dispatcher::backend::RepoBackend;

use crate::config::ZbobrRepoBackendFsConfig;

/// Serializable PR structure for YAML storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrFile {
    id: u64,
    repo: String,
    head_branch: String,
    base_branch: String,
    title: String,
    body: String,
    created_at: String,
}

/// Filesystem-based repo backend.
///
/// - `target_repo` is a local path to a git repository.
/// - "Forking" is done by `git clone` from the local path.
/// - When `repos_base_dir` is configured, a single main clone is maintained there,
///   and worktrees are created for each task/branch in the workspace.
/// - PRs are stored as YAML files under `{repos_dir}/prs/{repo_name}/`.
pub struct FilesystemRepoBackend {
    config: ZbobrRepoBackendFsConfig,
}

impl FilesystemRepoBackend {
    pub fn new(
        toml: Option<crate::config::ZbobrRepoBackendFsToml>,
        args: crate::config::ZbobrRepoBackendFsArgs,
        config_dir: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let config = ZbobrRepoBackendFsConfig::build(toml, args, config_dir);
        config.validate()?;
        Ok(Self { config })
    }

    /// Extract a short repo name from a local path (last path component).
    fn repo_name_from_path(target_repo: &str) -> anyhow::Result<String> {
        let path = Path::new(target_repo);
        let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            anyhow::anyhow!("Cannot extract repo name from path: {}", target_repo)
        })?;
        Ok(name.to_string())
    }

    /// Get the prs directory for a given repo name.
    fn prs_dir(&self, repo_name: &str) -> PathBuf {
        self.config.repos_dir.join("prs").join(repo_name)
    }

    /// Get the main clone directory for a given repo (persistent).
    fn main_clone_dir(&self, repo_name: &str) -> Option<PathBuf> {
        self.config
            .repos_base_dir
            .as_ref()
            .map(|base| base.join(repo_name))
    }

    /// Get the worktree directory path for a branch.
    fn worktree_dir(workspace_path: &Path, repo_name: &str) -> PathBuf {
        workspace_path.join(repo_name)
    }

    /// Read and increment the next PR ID counter for a repo.
    async fn get_next_pr_id(&self, repo_name: &str) -> anyhow::Result<u64> {
        let prs_dir = self.prs_dir(repo_name);
        fs::create_dir_all(&prs_dir)
            .await
            .context("Failed to create prs directory")?;

        let path = prs_dir.join("next_pr_id.txt");

        let current_id = match fs::read_to_string(&path).await {
            Ok(content) => content.trim().parse::<u64>().unwrap_or(1),
            Err(_) => 1,
        };

        let next_id = current_id + 1;
        fs::write(&path, next_id.to_string())
            .await
            .context("Failed to write next PR ID")?;

        Ok(current_id)
    }

    /// Ensure main clone exists and is up-to-date.
    async fn ensure_main_clone(
        &self,
        target_repo: &str,
        repo_name: &str,
    ) -> anyhow::Result<PathBuf> {
        let main_clone = self
            .main_clone_dir(repo_name)
            .context("repos_base_dir not configured")?;

        fs::create_dir_all(&main_clone).await?;

        if !main_clone.join("HEAD").exists() {
            tracing::info!("Creating bare main clone for {} at {}", repo_name, main_clone.display());
            let status = tokio::process::Command::new("git")
                .args(["clone", "--bare", target_repo, main_clone.to_str().unwrap()])
                .status()
                .await?;
            if !status.success() {
                anyhow::bail!("Failed to clone {} into main clone", target_repo);
            }
        } else {
            tracing::info!("Updating main clone for {} at {}", repo_name, main_clone.display());
            let fetch_status = tokio::process::Command::new("git")
                .args(["fetch", "origin", "refs/heads/*:refs/heads/*"])
                .current_dir(&main_clone)
                .status()
                .await?;
            if !fetch_status.success() {
                tracing::warn!(
                    "Failed to fetch latest changes for main clone {}, using existing state",
                    repo_name
                );
            }
        }

        Ok(main_clone)
    }

    /// Create or attach a worktree for a specific branch.
    async fn create_or_attach_worktree(
        &self,
        main_clone: &Path,
        worktree_path: &Path,
        branch: &str,
    ) -> anyhow::Result<()> {
        // Clean up any stale worktree at the target path
        if worktree_path.exists() {
            tracing::info!("Removing existing worktree at {}", worktree_path.display());
            let remove_status = tokio::process::Command::new("git")
                .args(["worktree", "remove", "--force", worktree_path.to_str().unwrap()])
                .current_dir(main_clone)
                .status()
                .await?;
            if !remove_status.success() {
                tracing::warn!(
                    "Failed to remove worktree, trying to continue anyway"
                );
            }
        }

        // Create the worktree
        let status = tokio::process::Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                branch,
            ])
            .current_dir(main_clone)
            .status()
            .await?;

        if !status.success() {
            // If branch doesn't exist, create it from HEAD
            tracing::info!("Creating new branch {} in worktree", branch);
            let create_status = tokio::process::Command::new("git")
                .args([
                    "worktree",
                    "add",
                    "-b",
                    branch,
                    worktree_path.to_str().unwrap(),
                ])
                .current_dir(main_clone)
                .status()
                .await?;
            if !create_status.success() {
                anyhow::bail!("Failed to create worktree for branch {}", branch);
            }
        }

        Ok(())
    }

    /// Write a PR YAML file and return the file path.
    async fn write_pr(
        &self,
        repo_name: &str,
        repo_path: &str,
        head_branch: &str,
        base_branch: &str,
        title: &str,
        body: &str,
    ) -> anyhow::Result<String> {
        let pr_id = self.get_next_pr_id(repo_name).await?;
        let prs_dir = self.prs_dir(repo_name);
        let pr_path = prs_dir.join(format!("{}.yaml", pr_id));

        let pr_file = PrFile {
            id: pr_id,
            repo: repo_path.to_string(),
            head_branch: head_branch.to_string(),
            base_branch: base_branch.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            created_at: chrono_now(),
        };

        let yaml = serde_yaml::to_string(&pr_file).context("Failed to serialize PR")?;

        fs::write(&pr_path, yaml)
            .await
            .context("Failed to write PR file")?;

        tracing::info!("Created PR #{} at {}", pr_id, pr_path.display());

        Ok(pr_path.to_string_lossy().to_string())
    }

    /// Get the current branch name in a git working directory.
    async fn current_branch(work_dir: &Path) -> anyhow::Result<String> {
        let out = tokio::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(work_dir)
            .output()
            .await
            .context("Failed to determine current branch")?;

        if !out.status.success() {
            anyhow::bail!("Failed to determine current branch");
        }

        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

/// Simple timestamp without pulling in chrono crate.
fn chrono_now() -> String {
    // Use a basic approach — the exact format is not critical for local PRs
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[async_trait]
impl RepoBackend for FilesystemRepoBackend {
    async fn clone_and_setup(
        &self,
        target_repo: &str,
        branch: &str,
        workspace_path: &Path,
    ) -> anyhow::Result<PathBuf> {
        let repo_name = Self::repo_name_from_path(target_repo)?;
        let work_dir = Self::worktree_dir(workspace_path, &repo_name);

        fs::create_dir_all(workspace_path).await?;

        // If repos_base_dir is configured, use worktrees
        if self.config.repos_base_dir.is_some() {
            let main_clone = self.ensure_main_clone(target_repo, &repo_name).await?;
            self.create_or_attach_worktree(&main_clone, &work_dir, branch)
                .await?;
        } else {
            // Legacy mode: use full clone
            if !work_dir.exists() {
                tracing::info!("Cloning {} into {}", target_repo, work_dir.display());
                let status = tokio::process::Command::new("git")
                    .args(["clone", target_repo, work_dir.to_str().unwrap()])
                    .status()
                    .await?;
                if !status.success() {
                    anyhow::bail!("Failed to clone {}", target_repo);
                }
            } else {
                tracing::info!("Updating {} in {}", target_repo, work_dir.display());
                let fetch_status = tokio::process::Command::new("git")
                    .args(["fetch", "origin"])
                    .current_dir(&work_dir)
                    .status()
                    .await?;
                if !fetch_status.success() {
                    tracing::warn!(
                        "Failed to fetch latest changes for {}, using existing state",
                        target_repo
                    );
                }
            }

            // Checkout the requested branch; create from HEAD if it doesn't exist yet.
            tracing::info!("Checking out branch {}", branch);
            let checkout_status = tokio::process::Command::new("git")
                .args(["checkout", branch])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !checkout_status.success() {
                let create_status = tokio::process::Command::new("git")
                    .args(["checkout", "-b", branch])
                    .current_dir(&work_dir)
                    .status()
                    .await?;
                if !create_status.success() {
                    anyhow::bail!("Failed to checkout or create branch {}", branch);
                }
            }
        }

        Ok(work_dir)
    }

    async fn clone_readonly(
        &self,
        target_repo: &str,
        branch: &str,
        workspace_path: &Path,
    ) -> anyhow::Result<PathBuf> {
        // In FS mode, clone_readonly is identical to clone_and_setup
        // (no fork concept to skip)
        self.clone_and_setup(target_repo, branch, workspace_path)
            .await
    }

    async fn setup_fork_remote_and_push(
        &self,
        work_dir: &Path,
        _target_repo: &str,
        work_branch: &str,
    ) -> anyhow::Result<()> {
        // In FS mode, origin already points to the local source repo.
        // Just push the work branch.
        tracing::info!("Pushing work branch '{}' to origin", work_branch);
        let push_status = tokio::process::Command::new("git")
            .args(["push", "-u", "origin", work_branch])
            .current_dir(work_dir)
            .status()
            .await?;

        if !push_status.success() {
            anyhow::bail!("Failed to push work branch '{}' to origin", work_branch);
        }

        Ok(())
    }

    async fn ensure_branch_and_pr(
        &self,
        target_repo: &str,
        workspace_path: &Path,
        _work_branch: &str,
        _destination_branch: &str,
        _pr_title: &str,
    ) -> anyhow::Result<String> {
        let repo_name = Self::repo_name_from_path(target_repo)?;
        let work_dir = Self::worktree_dir(workspace_path, &repo_name);

        if !work_dir.exists() {
            anyhow::bail!("Work directory does not exist: {}", work_dir.display());
        }

        // FS backend: return the work directory path as the "PR URL"
        Ok(work_dir.to_string_lossy().to_string())
    }

    async fn push_branch(
        &self,
        _target_repo: &str,
        _workspace_path: &Path,
        _work_branch: &str,
    ) -> anyhow::Result<()> {
        // FS backend: no-op
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
        // In FS mode, "fork" is just the local clone. Create the PR YAML.
        // repo_name here is just the short name (not a full path), so we store
        // the repo field as the repo_name — callers can correlate.
        self.write_pr(
            repo_name,
            repo_name,
            work_branch,
            destination_branch,
            pr_title,
            pr_body,
        )
        .await
    }

    async fn parse_pr_to_repo_branch(&self, pr_ref: &str) -> anyhow::Result<(String, String)> {
        // pr_ref is a path to a PR YAML file
        let content = fs::read_to_string(pr_ref)
            .await
            .with_context(|| format!("Failed to read PR file '{}'", pr_ref))?;

        let pr_file: PrFile = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse PR file '{}'", pr_ref))?;

        Ok((pr_file.repo, pr_file.head_branch))
    }

    async fn validate_connectivity(&self) -> anyhow::Result<()> {
        // Check that we can write to repos_dir
        fs::create_dir_all(&self.config.repos_dir)
            .await
            .with_context(|| {
                format!(
                    "Cannot access repos directory '{}'",
                    self.config.repos_dir.display()
                )
            })?;

        // Try to write a test file
        let test_path = self.config.repos_dir.join(".test");
        fs::write(&test_path, "test").await.with_context(|| {
            format!(
                "Cannot write to repos directory '{}'",
                self.config.repos_dir.display()
            )
        })?;

        // Clean up
        let _ = fs::remove_file(&test_path).await;

        tracing::info!("Filesystem repo backend connectivity validated");
        Ok(())
    }

    fn debug_state(&self) -> String {
        format!(
            "FilesystemRepoBackend(repos_dir: {})",
            self.config.repos_dir.display()
        )
    }
}

impl FilesystemRepoBackend {
    /// Remove a worktree if it exists (for cleanup).
    /// This method is called during task cleanup or when a stale worktree needs removal.
    pub async fn cleanup_worktree(
        &self,
        target_repo: &str,
        workspace_path: &Path,
    ) -> anyhow::Result<()> {
        // Only applicable when using worktrees
        if self.config.repos_base_dir.is_none() {
            return Ok(());
        }

        let repo_name = Self::repo_name_from_path(target_repo)?;
        let worktree_path = Self::worktree_dir(workspace_path, &repo_name);

        if !worktree_path.exists() {
            tracing::debug!("Worktree at {} does not exist, skipping cleanup", worktree_path.display());
            return Ok(());
        }

        let main_clone = self
            .main_clone_dir(&repo_name)
            .context("repos_base_dir not configured")?;

        if !main_clone.join(".git").exists() {
            tracing::warn!("Main clone does not exist, cannot remove worktree cleanly");
            return Ok(());
        }

        tracing::info!("Removing worktree at {}", worktree_path.display());
        let remove_status = tokio::process::Command::new("git")
            .args(["worktree", "remove", "--force", worktree_path.to_str().unwrap()])
            .current_dir(&main_clone)
            .status()
            .await?;

        if !remove_status.success() {
            tracing::warn!(
                "Failed to remove worktree at {}, attempting fallback cleanup",
                worktree_path.display()
            );
            // Fallback: try to remove the directory directly
            let _ = fs::remove_dir_all(&worktree_path).await;
        }

        Ok(())
    }

    /// Get the default branch of origin remote.
    async fn default_branch(work_dir: &Path) -> anyhow::Result<String> {
        let out = tokio::process::Command::new("git")
            .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
            .current_dir(work_dir)
            .output()
            .await
            .context("Failed to determine default branch")?;

        if !out.status.success() {
            anyhow::bail!("Failed to determine default branch");
        }

        let full_ref = String::from_utf8_lossy(&out.stdout).trim().to_string();
        // Strip "origin/" prefix
        let branch = full_ref.strip_prefix("origin/").unwrap_or(&full_ref);
        Ok(branch.to_string())
    }
}

use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use async_trait::async_trait;

use crate::{Model, Parameter, Stage, Task, Tool};

/// Configure git user settings for a repository.
/// Sets user.name and user.email at the repository level.
pub async fn configure_git_user(
    work_dir: &std::path::Path,
    git_user_name: &str,
    git_user_email: &str,
) -> anyhow::Result<()> {
    // Set git user configuration for this repository
    let config_user_status = tokio::process::Command::new("git")
        .args(["config", "--local", "user.name", git_user_name])
        .current_dir(work_dir)
        .status()
        .await
        .context("Failed to set git user.name")?;

    if !config_user_status.success() {
        anyhow::bail!("git config user.name failed");
    }

    let config_email_status = tokio::process::Command::new("git")
        .args(["config", "--local", "user.email", git_user_email])
        .current_dir(work_dir)
        .status()
        .await
        .context("Failed to set git user.email")?;

    if !config_email_status.success() {
        anyhow::bail!("git config user.email failed");
    }

    tracing::info!(
        "Configured git user '{}' <{}> in {}",
        git_user_name,
        git_user_email,
        work_dir.display()
    );

    Ok(())
}

/// Create a placeholder file in a branch to ensure it has at least one commit.
/// This is used to prevent GitHub PR API from rejecting branches with no commits.
///
/// Creates `.zbobr/{branch_name}` file, stages it, and commits it.
/// Does NOT push — the caller is responsible for pushing if needed.
///
/// Git user configuration should be set up before calling this function (via configure_git_user).
///
/// # Arguments
/// * `work_dir` - The repository working directory
/// * `branch_name` - The branch name (used for file naming and commit message)
pub async fn create_placeholder_commit(
    work_dir: &std::path::Path,
    branch_name: &str,
) -> anyhow::Result<()> {
    zbobr_utility::create_placeholder_commit(work_dir, branch_name).await
}

/// TaskBackend: stores and manages tasks, their metadata, comments, and lifecycle.
///
/// Implementations:
/// - GitHub: Tasks as Issues, stages as Milestones, signals/tools/models as Labels
/// - Directory: Tasks as JSON files, stages as subdirectories
#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait TaskBackend: Send + Sync {
    // -- Core CRUD --

    /// Get a task by ID.
    async fn get_task(&self, id: u64) -> anyhow::Result<Task>;

    /// Create a new task. Returns the task ID.
    #[allow(clippy::too_many_arguments)]
    async fn create_task(
        &self,
        title: &str,
        description: &str,
        stage: Stage,
        tool: Option<Tool>,
        model: Option<Model>,
        parameters: HashMap<Parameter, String>,
    ) -> anyhow::Result<u64>;

    /// Close a task.
    async fn close_task(&self, id: u64) -> anyhow::Result<()>;

    /// Check if a task is closed.
    async fn is_task_closed(&self, id: u64) -> anyhow::Result<bool>;

    // -- Atomic modification --

    /// Read-modify-write the task atomically.
    ///
    /// Takes `Task` by value and returns the modified version to avoid
    /// reference lifetime issues with `async_trait`.
    async fn modify_task(
        &self,
        id: u64,
        mutate: Box<dyn FnOnce(Task) -> Task + Send>,
    ) -> anyhow::Result<()>;

    // -- Queries --

    /// List open tasks with a given stage, optionally filtered by tool.
    async fn list_tasks_by_stage(
        &self,
        stage: Stage,
        tool: Option<Tool>,
    ) -> anyhow::Result<Vec<Task>>;

    // -- Discussion --

    /// Get all comments on a task as formatted discussion.
    async fn get_task_comments(&self, id: u64) -> anyhow::Result<Vec<String>>;

    /// Post a comment on a task with role and hostname metadata.
    async fn post_task_comment(
        &self,
        id: u64,
        body: &str,
        role: &str,
        hostname: &str,
    ) -> anyhow::Result<()>;

    // -- Lifecycle --

    /// Initialize storage with required stages, labels, etc.
    /// If force is true, overwrites existing labels.
    async fn setup(&self, force: bool) -> anyhow::Result<()>;

    /// Validate connectivity to the task storage.
    async fn validate_connectivity(&self) -> anyhow::Result<()>;

    /// Return a debug string of the backend state.
    fn debug_state(&self) -> String;
}

/// RepoBackend: manages code repositories — cloning, forking, branching, and PRs.
///
/// Implementations:
/// - GitHub: Forks via GitHub API, clones via `gh`, PRs via Pulls API
/// - Directory: Local git repos, direct push, no fork/PR concept
#[async_trait]
pub trait RepoBackend: Send + Sync {
    // -- Clone/checkout --

    /// Clone a repo into the workspace, checkout work branch, sync destination branch, set up fork remote.
    /// Ensures the fork is synced with destination_branch, the local destination_branch is force-reset
    /// to match origin, and the work_branch is checked out ready for merging.
    /// Returns the local path.
    async fn clone_and_setup(
        &self,
        target_repo: &str,
        work_branch: &str,
        destination_branch: &str,
        workspace_path: &std::path::Path,
    ) -> anyhow::Result<PathBuf>;

    /// Clone a repo and checkout specific branch for read-only investigation (no fork).
    async fn clone_readonly(
        &self,
        target_repo: &str,
        branch: &str,
        workspace_path: &std::path::Path,
    ) -> anyhow::Result<PathBuf>;

    // -- Fork management --

    /// Replace origin remote with the fork URL and push a work branch.
    /// The backend determines the fork owner and constructs the fork URL internally.
    async fn setup_fork_remote_and_push(
        &self,
        work_dir: &std::path::Path,
        target_repo: &str,
        work_branch: &str,
    ) -> anyhow::Result<()>;

    // -- PR operations --

    /// Create (or find existing) branch reference on the remote and return a PR URL.
    ///
    /// For FS backend: returns the work directory path string (no git push).
    /// For GitHub backend: pushes the branch (adding a placeholder commit if the branch has no
    /// new commits relative to `destination_branch`), creates a draft PR, or finds the existing
    /// PR URL if GitHub returns 422, and returns the PR `html_url`.
    async fn ensure_branch_and_pr(
        &self,
        target_repo: &str,
        workspace_path: &std::path::Path,
        work_branch: &str,
        destination_branch: &str,
        pr_title: &str,
    ) -> anyhow::Result<String>;

    /// Push current work branch commits to the remote (fork or origin).
    ///
    /// FS backend: no-op (`Ok(())`).
    /// GitHub backend: git push to fork or origin (same remote detection as `ensure_branch_and_pr`).
    async fn push_branch(
        &self,
        target_repo: &str,
        workspace_path: &std::path::Path,
        work_branch: &str,
    ) -> anyhow::Result<()>;

    /// Create a PR from work_branch to destination_branch in the fork repo.
    /// `repo_name` is just the repository name (e.g. "myrepo"), not a full "owner/repo" path.
    /// The backend determines the fork owner internally.
    /// `pr_title` and `pr_body` are provided by the caller (decoupled from task storage).
    /// Returns the PR URL.
    async fn create_pr_in_fork(
        &self,
        repo_name: &str,
        work_branch: &str,
        destination_branch: &str,
        pr_title: &str,
        pr_body: &str,
    ) -> anyhow::Result<String>;

    /// Parse PR reference (URL or owner/repo#123) to (repo, branch).
    async fn parse_pr_to_repo_branch(&self, pr_ref: &str) -> anyhow::Result<(String, String)>;

    // -- Lifecycle --

    /// Validate connectivity to the repo hosting service (fork owner accessible, etc.).
    async fn validate_connectivity(&self) -> anyhow::Result<()>;

    /// Return a debug string of the backend state.
    fn debug_state(&self) -> String;
}

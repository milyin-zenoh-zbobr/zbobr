use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use tempfile::TempDir;
use zbobr_dispatcher::backend::RepoBackend;
use zbobr_repo_backend_fs::FilesystemRepoBackend;

/// Test harness that holds a temporary directory, a bare "source" git repo, and
/// a configured filesystem repo backend.
pub struct TestSetup {
    /// Keep alive so the temp directory is not deleted while tests run.
    pub _tmp: TempDir,
    /// Path to the bare git repository (acts as remote origin).
    pub source_repo: PathBuf,
    /// Path to the repos_dir used by the backend.
    pub repos_dir: PathBuf,
    /// The backend under test.
    pub backend: Arc<dyn RepoBackend>,
}

/// Return the source repo path as a string (needed for `target_repo: &str` args).
pub fn source_repo_str(setup: &TestSetup) -> String {
    setup.source_repo.to_str().unwrap().to_string()
}

/// Return a unique workspace path under the temp directory.
pub fn workspace_path(setup: &TestSetup, name: &str) -> PathBuf {
    setup._tmp.path().join("workspaces").join(name)
}

/// Create a fresh test setup with a bare git repo and a filesystem repo backend.
pub async fn create_test_setup() -> TestSetup {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let base = tmp.path();

    let bare_repo = base.join("source.git");
    let staging = base.join("staging");
    let repos_dir = base.join("repos");

    // 1. Create bare repo
    git_command_status(&base, &["init", "--bare", bare_repo.to_str().unwrap()]).await;

    // 2. Clone bare repo into staging area
    git_command_status(
        &base,
        &[
            "clone",
            bare_repo.to_str().unwrap(),
            staging.to_str().unwrap(),
        ],
    )
    .await;

    // 3. Configure git user in staging
    git_command_status(&staging, &["config", "user.name", "Test User"]).await;
    git_command_status(&staging, &["config", "user.email", "test@test.com"]).await;

    // 4. Create initial commit on main
    git_command_status(&staging, &["checkout", "-b", "main"]).await;
    tokio::fs::write(staging.join("README.md"), "# Test Repo\n")
        .await
        .expect("write README.md");
    git_command_status(&staging, &["add", "README.md"]).await;
    git_command_status(&staging, &["commit", "-m", "initial commit"]).await;

    // 5. Push main to bare repo
    git_command_status(&staging, &["push", "-u", "origin", "main"]).await;

    // 6. Set bare repo HEAD to point to main
    git_command_status(&bare_repo, &["symbolic-ref", "HEAD", "refs/heads/main"]).await;

    // 7. Create backend
    let backend = FilesystemRepoBackend::new(
        None,
        zbobr_repo_backend_fs::ZbobrRepoBackendFsArgs {
            repos_dir: Some(repos_dir.to_path_buf()),
            repos_base_dir: None,
        },
        base,
    )
    .expect("failed to create fs backend");

    TestSetup {
        _tmp: tmp,
        source_repo: bare_repo,
        repos_dir,
        backend: Arc::new(backend),
    }
}

/// Create a fresh test setup with worktrees enabled.
pub async fn create_test_setup_with_worktrees() -> TestSetup {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let base = tmp.path();

    let bare_repo = base.join("source.git");
    let staging = base.join("staging");
    let repos_dir = base.join("repos");
    let repos_base_dir = base.join("repos_base");

    // 1. Create bare repo
    git_command_status(&base, &["init", "--bare", bare_repo.to_str().unwrap()]).await;

    // 2. Clone bare repo into staging area
    git_command_status(
        &base,
        &[
            "clone",
            bare_repo.to_str().unwrap(),
            staging.to_str().unwrap(),
        ],
    )
    .await;

    // 3. Configure git user in staging
    git_command_status(&staging, &["config", "user.name", "Test User"]).await;
    git_command_status(&staging, &["config", "user.email", "test@test.com"]).await;

    // 4. Create initial commit on main
    git_command_status(&staging, &["checkout", "-b", "main"]).await;
    tokio::fs::write(staging.join("README.md"), "# Test Repo\n")
        .await
        .expect("write README.md");
    git_command_status(&staging, &["add", "README.md"]).await;
    git_command_status(&staging, &["commit", "-m", "initial commit"]).await;

    // 5. Push main to bare repo
    git_command_status(&staging, &["push", "-u", "origin", "main"]).await;

    // 6. Set bare repo HEAD to point to main
    git_command_status(&bare_repo, &["symbolic-ref", "HEAD", "refs/heads/main"]).await;

    // 7. Create backend with worktrees enabled
    let backend = FilesystemRepoBackend::new(
        None,
        zbobr_repo_backend_fs::ZbobrRepoBackendFsArgs {
            repos_dir: Some(repos_dir.to_path_buf()),
            repos_base_dir: Some(repos_base_dir.to_path_buf()),
        },
        base,
    )
    .expect("failed to create fs backend with worktrees");

    TestSetup {
        _tmp: tmp,
        source_repo: bare_repo,
        repos_dir,
        backend: Arc::new(backend),
    }
}

/// Run a git command, assert success, and return trimmed stdout.
pub async fn git_command(dir: &Path, args: &[&str]) -> String {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .expect("git command failed to execute");
    assert!(
        output.status.success(),
        "git {:?} in {} failed: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Run a git command and assert success (ignore stdout).
pub async fn git_command_status(dir: &Path, args: &[&str]) {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .expect("git command failed to execute");
    assert!(
        output.status.success(),
        "git {:?} in {} failed: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Create a work branch with a commit in a cloned repo directory.
pub async fn create_work_branch(clone_dir: &Path, branch_name: &str) {
    git_command_status(clone_dir, &["config", "user.name", "Test User"]).await;
    git_command_status(clone_dir, &["config", "user.email", "test@test.com"]).await;
    git_command_status(clone_dir, &["checkout", "-b", branch_name]).await;
    tokio::fs::write(clone_dir.join("work.txt"), "work content\n")
        .await
        .expect("write work.txt");
    git_command_status(clone_dir, &["add", "work.txt"]).await;
    git_command_status(clone_dir, &["commit", "-m", "work commit"]).await;
}

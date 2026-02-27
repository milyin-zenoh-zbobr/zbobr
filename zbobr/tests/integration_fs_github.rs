/// Integration tests: filesystem task backend + GitHub repo backend.
///
/// Individual test functions are marked `#[ignore]` by default; run them
/// explicitly after adding your credentials using
/// `cargo test --test integration_fs_github -- --ignored`.
///
/// Activated when `zbobr_github_test.toml` at the workspace root contains a
/// `[repo.github]` section with valid credentials.
/// Run this group with: `cargo test --test integration_fs_github`
/// or filter by prefix: `cargo test test_fs_github_`
mod mcp_integration;

use std::sync::Arc;
use tokio::sync::OnceCell;

use mcp_integration::IntegrationTestEnv;
use mcp_integration::env::{RepoBackendArgs, TaskBackendArgs};
use mcp_integration::github_config::GitHubTestConfig;
use mcp_integration::test_helpers;

// panicking version: missing configuration is considered an error
static ENV: OnceCell<Arc<IntegrationTestEnv>> = OnceCell::const_new();
static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn get_env() -> Arc<IntegrationTestEnv> {
    ENV.get_or_init(|| async {
        let cfg = GitHubTestConfig::load()
            .expect("zbobr_github_test.toml not found; required for GitHub tests");
        let repo = cfg.repo
            .expect("[repo.github] section missing in zbobr_github_test.toml");

        let base = match std::env::var("CARGO_TARGET_TMPDIR") {
            Ok(p) => std::path::PathBuf::from(p).join("integration_env_fs_github"),
            Err(_) => std::env::temp_dir().join("zbobr_integration_env_fs_github"),
        };
        let tasks_dir = base.join("tasks");

        let target_repo = cfg.tasks.as_ref().map(|t| t.github.task_repo.clone());
        IntegrationTestEnv::init(
            "fs_github",
            TaskBackendArgs::Filesystem { tasks_dir },
            RepoBackendArgs::GitHub {
                fork_owner: repo.github.fork_owner,
                repo_token: repo.github.token,
            },
            cfg.dispatcher.agent_token,
            target_repo,
        )
        .await
        .expect("failed to initialize FS/GitHub environment; check credentials")
    })
    .await
    .clone()
}

// ---------------------------------------------------------------------------
// Basic workflow tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_preparation() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_preparation(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_planning() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_planning(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_working() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_working(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_reviewing() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_reviewing(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_merging() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_merging(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_merging_with_real_conflict() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_merging_with_real_conflict(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_conflict_detection() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_conflict_detection(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_reviewing_approval() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_reviewing_approval(&env).await;
}

// ---------------------------------------------------------------------------
// GitHub repo backend tests — same-org (no fork remote expected)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml with [tasks.github]"]
async fn test_fs_github_repo_backend_clone() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_clone(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml with [tasks.github]"]
async fn test_fs_github_repo_backend_planning() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_planning(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml with [tasks.github]"]
async fn test_fs_github_repo_backend_working() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_working(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml with [tasks.github]"]
async fn test_fs_github_repo_backend_reviewing() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_reviewing(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml with [tasks.github]"]
async fn test_fs_github_repo_backend_merging() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_merging(&env).await;
}

// ---------------------------------------------------------------------------
// GitHub repo backend tests — cross-org (fork remote expected)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_repo_backend_clone_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_clone_cross_org(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_repo_backend_planning_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_planning_cross_org(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_repo_backend_working_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_working_cross_org(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_repo_backend_reviewing_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_reviewing_cross_org(&env).await;
}

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn test_fs_github_repo_backend_merging_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_merging_cross_org(&env).await;
}

// ---------------------------------------------------------------------------
// Confirm flag behaviour
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "GitHub-backed test; requires zbobr_github_test.toml"]
async fn cli_confirm_flag_pauses_on_stage_change() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_cli_confirm_flag(&env).await;
}

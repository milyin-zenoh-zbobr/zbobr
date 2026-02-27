/// Integration tests: GitHub task backend + GitHub repo backend.
///
/// All tests in this file are marked `#[ignore]` and will not run unless
/// explicitly requested with `cargo test -- --ignored` or
/// `cargo test --test integration_github_github -- --ignored`.
///
/// Requires `zbobr_github_test.toml` to have both `[tasks.github]` and
/// `[repo.github]` sections with valid credentials.
mod mcp_integration;

use std::sync::Arc;
use tokio::sync::OnceCell;

use mcp_integration::IntegrationTestEnv;
use mcp_integration::env::{RepoBackendArgs, TaskBackendArgs};
use mcp_integration::github_config::GitHubTestConfig;
use mcp_integration::test_helpers;

// GitHub/GitHub tests are always ignored by default; missing config should
// cause a hard failure when run explicitly.
static ENV: OnceCell<Arc<IntegrationTestEnv>> = OnceCell::const_new();
static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn get_env() -> Arc<IntegrationTestEnv> {
    ENV.get_or_init(|| async {
        let cfg = GitHubTestConfig::load()
            .expect("zbobr_github_test.toml not found; required for GitHub tests");
        let tasks = cfg.tasks
            .expect("[tasks.github] section missing in zbobr_github_test.toml");
        let repo = cfg.repo
            .expect("[repo.github] section missing in zbobr_github_test.toml");

        let target_repo = Some(tasks.github.task_repo.clone());
        IntegrationTestEnv::init(
            "github_github",
            TaskBackendArgs::GitHub {
                task_repo: tasks.github.task_repo,
                task_token: tasks.github.token,
            },
            RepoBackendArgs::GitHub {
                fork_owner: repo.github.fork_owner,
                repo_token: repo.github.token,
            },
            cfg.dispatcher.agent_token,
            target_repo,
        )
        .await
        .expect("failed to initialize full GitHub/GitHub integration environment; check credentials")
    })
    .await
    .clone()
}

// ---------------------------------------------------------------------------
// Tests (all ignored by default)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_preparation() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_preparation(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_planning() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_planning(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_working() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_working(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_reviewing() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_reviewing(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_merging() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_merging(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_merging_with_real_conflict() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_merging_with_real_conflict(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_conflict_detection() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_conflict_detection(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_reviewing_approval() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_reviewing_approval(&env).await;
}

// ---------------------------------------------------------------------------
// GitHub repo backend tests — same-org
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_clone() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_clone(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_planning() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_planning(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_working() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_working(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_reviewing() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_reviewing(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_merging() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_merging(&env).await;
}

// ---------------------------------------------------------------------------
// GitHub repo backend tests — cross-org
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_clone_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_clone_cross_org(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_planning_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_planning_cross_org(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_working_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_working_cross_org(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_reviewing_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_reviewing_cross_org(&env).await;
}

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_repo_backend_merging_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_repo_backend_merging_cross_org(&env).await;
}

// ---------------------------------------------------------------------------
// Confirm flag behaviour
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "full GitHub backend test — run with `cargo test -- --ignored`"]
async fn test_github_github_cli_confirm_flag_pauses_on_stage_change() {
    let _guard = TEST_LOCK.lock().await;
    let env = get_env().await;
    test_helpers::run_cli_confirm_flag(&env).await;
}

/// Integration tests: filesystem task backend + filesystem repo backend.
///
/// These tests are always active (no GitHub credentials required).
/// Run this group with: `cargo test --test integration_fs_fs`
/// or filter by prefix: `cargo test test_fs_fs_`
mod mcp_integration;

use std::sync::Arc;
use tokio::sync::OnceCell;

use mcp_integration::IntegrationTestEnv;
use mcp_integration::env::{RepoBackendArgs, TaskBackendArgs};
use mcp_integration::test_helpers;

static ENV: OnceCell<Option<Arc<IntegrationTestEnv>>> = OnceCell::const_new();
static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn get_env() -> Option<Arc<IntegrationTestEnv>> {
    ENV.get_or_init(|| async {
        let base = match std::env::var("CARGO_TARGET_TMPDIR") {
            Ok(p) => std::path::PathBuf::from(p).join("integration_env_fs_fs"),
            Err(_) => std::env::temp_dir().join("zbobr_integration_env_fs_fs"),
        };
        let tasks_dir = base.join("tasks");

        IntegrationTestEnv::init(
            "fs_fs",
            TaskBackendArgs::Filesystem { tasks_dir },
            RepoBackendArgs::Filesystem,
            "dummy-not-used".to_string(),
            None,
        )
        .await
    })
    .await
    .clone()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fs_fs_preparation() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_preparation(&env).await;
}

#[tokio::test]
async fn test_fs_fs_planning() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_planning(&env).await;
}

#[tokio::test]
async fn test_fs_fs_working() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_working(&env).await;
}

#[tokio::test]
async fn test_fs_fs_reviewing() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_reviewing(&env).await;
}

#[tokio::test]
async fn test_fs_fs_merging() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_merging(&env).await;
}

#[tokio::test]
async fn test_fs_fs_merging_with_real_conflict() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_merging_with_real_conflict(&env).await;
}

#[tokio::test]
async fn test_fs_fs_conflict_detection() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_conflict_detection(&env).await;
}

#[tokio::test]
async fn test_fs_fs_signal_preservation_during_conflict() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_signal_preservation_during_conflict(&env).await;
}

#[tokio::test]
async fn test_fs_fs_reviewing_approval() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_reviewing_approval(&env).await;
}

// ---------------------------------------------------------------------------
// GitHub repo backend tests — same-org (skipped: not a GitHub repo backend)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fs_fs_repo_backend_clone() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_clone(&env).await;
}

#[tokio::test]
async fn test_fs_fs_repo_backend_planning() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_planning(&env).await;
}

#[tokio::test]
async fn test_fs_fs_repo_backend_working() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_working(&env).await;
}

#[tokio::test]
async fn test_fs_fs_repo_backend_reviewing() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_reviewing(&env).await;
}

#[tokio::test]
async fn test_fs_fs_repo_backend_merging() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_merging(&env).await;
}

// ---------------------------------------------------------------------------
// GitHub repo backend tests — cross-org (skipped: not a GitHub repo backend)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fs_fs_repo_backend_clone_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_clone_cross_org(&env).await;
}

#[tokio::test]
async fn test_fs_fs_repo_backend_planning_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_planning_cross_org(&env).await;
}

#[tokio::test]
async fn test_fs_fs_repo_backend_working_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_working_cross_org(&env).await;
}

#[tokio::test]
async fn test_fs_fs_repo_backend_reviewing_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_reviewing_cross_org(&env).await;
}

#[tokio::test]
async fn test_fs_fs_repo_backend_merging_cross_org() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_repo_backend_merging_cross_org(&env).await;
}

// ---------------------------------------------------------------------------
// Confirm flag behaviour
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fs_fs_cli_confirm_flag_pauses_on_stage_change() {
    let _guard = TEST_LOCK.lock().await;
    let Some(env) = get_env().await else { return };
    test_helpers::run_cli_confirm_flag(&env).await;
}

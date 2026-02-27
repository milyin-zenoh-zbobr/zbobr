mod common;

use std::path::Path;

use common::{
    create_test_setup, create_test_setup_with_worktrees, create_work_branch, git_command,
    source_repo_str, workspace_path,
};

/// Deserialize PR YAML files in test assertions (mirrors the private `PrFile`).
#[derive(Debug, serde::Deserialize)]
struct TestPrFile {
    id: u64,
    repo: String,
    head_branch: String,
    base_branch: String,
    title: String,
    body: String,
    created_at: String,
}

// ---------------------------------------------------------------------------
// Basic operations
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_validate_connectivity() {
    let setup = create_test_setup().await;
    setup
        .backend
        .validate_connectivity()
        .await
        .expect("validate_connectivity should succeed");
}

#[tokio::test]
async fn test_debug_state() {
    let setup = create_test_setup().await;
    let state = setup.backend.debug_state();
    assert!(
        state.contains("FilesystemRepoBackend"),
        "debug_state should contain backend name, got: {state}"
    );
    assert!(
        state.contains(setup.repos_dir.to_str().unwrap()),
        "debug_state should contain repos_dir path, got: {state}"
    );
}

// ---------------------------------------------------------------------------
// Clone operations
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_clone_and_setup() {
    let setup = create_test_setup().await;
    let ws = workspace_path(&setup, "clone_test");
    let src = source_repo_str(&setup);

    let clone_dir = setup
        .backend
        .clone_and_setup(&src, "main", "main", &ws)
        .await
        .expect("clone_and_setup should succeed");

    // Returned path should be workspace/source.git
    assert_eq!(clone_dir, ws.join("source.git"));
    // Should contain a .git directory
    assert!(clone_dir.join(".git").exists(), ".git dir should exist");
    // Should contain the committed file
    assert!(
        clone_dir.join("README.md").exists(),
        "README.md should exist"
    );
    // Current branch should be main
    let branch = git_command(&clone_dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert_eq!(branch, "main");
}

#[tokio::test]
async fn test_clone_and_setup_existing_dir() {
    let setup = create_test_setup().await;
    let ws = workspace_path(&setup, "clone_existing");
    let src = source_repo_str(&setup);

    // First clone
    let dir1 = setup
        .backend
        .clone_and_setup(&src, "main", "main", &ws)
        .await
        .expect("first clone should succeed");

    // Second clone — should fetch instead of re-cloning
    let dir2 = setup
        .backend
        .clone_and_setup(&src, "main", "main", &ws)
        .await
        .expect("second clone should succeed");

    assert_eq!(dir1, dir2, "both calls should return the same path");

    let branch = git_command(&dir2, &["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert_eq!(branch, "main");
}

#[tokio::test]
async fn test_clone_readonly() {
    let setup = create_test_setup().await;
    let ws = workspace_path(&setup, "clone_readonly");
    let src = source_repo_str(&setup);

    let clone_dir = setup
        .backend
        .clone_readonly(&src, "main", &ws)
        .await
        .expect("clone_readonly should succeed");

    assert!(clone_dir.join(".git").exists(), ".git dir should exist");
    assert!(
        clone_dir.join("README.md").exists(),
        "README.md should exist"
    );
}

#[tokio::test]
async fn test_clone_nonexistent_branch() {
    let setup = create_test_setup().await;
    let ws = workspace_path(&setup, "clone_noexist");
    let src = source_repo_str(&setup);

    let result = setup
        .backend
        .clone_and_setup(&src, "nonexistent-branch", "main", &ws)
        .await;

    // Nonexistent branches are created automatically from the current HEAD.
    assert!(
        result.is_ok(),
        "cloning nonexistent branch should succeed and create branch"
    );

    let repo_name = Path::new(&src).file_name().unwrap().to_str().unwrap();
    let worktree_path = ws.join(repo_name);
    assert!(worktree_path.exists(), "worktree should exist");

    let output = std::process::Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(&worktree_path)
        .output()
        .expect("failed to get current branch");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        branch, "nonexistent-branch",
        "worktree should be on the new branch"
    );
}

// ---------------------------------------------------------------------------
// Push operations
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_setup_fork_remote_and_push() {
    let setup = create_test_setup().await;
    let ws = workspace_path(&setup, "push_test");
    let src = source_repo_str(&setup);

    // Clone
    let clone_dir = setup
        .backend
        .clone_and_setup(&src, "main", "main", &ws)
        .await
        .expect("clone should succeed");

    // Create a work branch with a commit
    create_work_branch(&clone_dir, "work-branch-1").await;

    // Push it
    setup
        .backend
        .setup_fork_remote_and_push(&clone_dir, &src, "work-branch-1")
        .await
        .expect("setup_fork_remote_and_push should succeed");

    // Verify the branch exists in the bare source repo
    let branches = git_command(&setup.source_repo, &["branch", "--list", "work-branch-1"]).await;
    assert!(
        branches.contains("work-branch-1"),
        "work-branch-1 should exist in bare repo, got: {branches}"
    );
}

// ---------------------------------------------------------------------------
// PR operations
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_pr_in_fork() {
    let setup = create_test_setup().await;

    let pr_path = setup
        .backend
        .create_pr_in_fork(
            "myrepo",
            "feature-branch",
            "main",
            "My PR Title",
            "My PR body",
        )
        .await
        .expect("create_pr_in_fork should succeed");

    // File should exist
    assert!(
        Path::new(&pr_path).exists(),
        "PR file should exist at {pr_path}"
    );

    // Read and verify contents
    let content = tokio::fs::read_to_string(&pr_path)
        .await
        .expect("read PR file");
    let pr: TestPrFile = serde_yaml::from_str(&content).expect("parse PR YAML");

    assert_eq!(pr.id, 1);
    assert_eq!(pr.repo, "myrepo");
    assert_eq!(pr.head_branch, "feature-branch");
    assert_eq!(pr.base_branch, "main");
    assert_eq!(pr.title, "My PR Title");
    assert_eq!(pr.body, "My PR body");
    assert!(!pr.created_at.is_empty());
}

#[tokio::test]
async fn test_push_branch_and_create_pr() {
    let setup = create_test_setup().await;
    let ws = workspace_path(&setup, "pr_push_test");
    let src = source_repo_str(&setup);

    // Clone and create work branch
    let clone_dir = setup
        .backend
        .clone_and_setup(&src, "main", "main", &ws)
        .await
        .expect("clone should succeed");

    create_work_branch(&clone_dir, "pr-branch").await;

    setup
        .backend
        .setup_fork_remote_and_push(&clone_dir, &src, "pr-branch")
        .await
        .expect("setup_fork_remote_and_push should succeed");

    let repo_name = Path::new(&src)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("repo name");

    let pr_path = setup
        .backend
        .create_pr_in_fork(repo_name, "pr-branch", "main", "PR Title", "PR Body")
        .await
        .expect("create_pr_in_fork should succeed");

    // Verify PR file
    let content = tokio::fs::read_to_string(&pr_path)
        .await
        .expect("read PR file");
    let pr: TestPrFile = serde_yaml::from_str(&content).expect("parse PR YAML");

    assert_eq!(pr.head_branch, "pr-branch");
    assert_eq!(pr.base_branch, "main");
    assert_eq!(pr.title, "PR Title");
    assert_eq!(pr.body, "PR Body");

    // Verify the branch was pushed to the bare repo
    let branches = git_command(&setup.source_repo, &["branch", "--list", "pr-branch"]).await;
    assert!(
        branches.contains("pr-branch"),
        "pr-branch should exist in bare repo"
    );
}

#[tokio::test]
async fn test_parse_pr_to_repo_branch() {
    let setup = create_test_setup().await;

    // Create a PR first
    let pr_path = setup
        .backend
        .create_pr_in_fork("myrepo", "feature-x", "main", "title", "body")
        .await
        .expect("create_pr_in_fork should succeed");

    // Parse it back
    let (repo, branch) = setup
        .backend
        .parse_pr_to_repo_branch(&pr_path)
        .await
        .expect("parse_pr_to_repo_branch should succeed");

    assert_eq!(repo, "myrepo");
    assert_eq!(branch, "feature-x");
}

#[tokio::test]
async fn test_pr_id_auto_increment() {
    let setup = create_test_setup().await;

    let pr_path_1 = setup
        .backend
        .create_pr_in_fork("myrepo", "branch-1", "main", "PR 1", "body 1")
        .await
        .expect("first PR");

    let pr_path_2 = setup
        .backend
        .create_pr_in_fork("myrepo", "branch-2", "main", "PR 2", "body 2")
        .await
        .expect("second PR");

    // Verify IDs
    let pr1: TestPrFile =
        serde_yaml::from_str(&tokio::fs::read_to_string(&pr_path_1).await.unwrap()).unwrap();
    let pr2: TestPrFile =
        serde_yaml::from_str(&tokio::fs::read_to_string(&pr_path_2).await.unwrap()).unwrap();

    assert_eq!(pr1.id, 1);
    assert_eq!(pr2.id, 2);

    // Verify file names
    assert!(pr_path_1.ends_with("1.yaml"), "first PR should be 1.yaml");
    assert!(pr_path_2.ends_with("2.yaml"), "second PR should be 2.yaml");

    // Verify next_pr_id.txt
    let next_id_path = setup
        .repos_dir
        .join("prs")
        .join("myrepo")
        .join("next_pr_id.txt");
    let next_id = tokio::fs::read_to_string(&next_id_path)
        .await
        .expect("read next_pr_id.txt");
    assert_eq!(next_id.trim(), "3");
}

#[tokio::test]
async fn test_pr_id_separate_repos() {
    let setup = create_test_setup().await;

    let pr_a = setup
        .backend
        .create_pr_in_fork("repo-a", "branch-a", "main", "PR A", "body A")
        .await
        .expect("PR in repo-a");

    let pr_b = setup
        .backend
        .create_pr_in_fork("repo-b", "branch-b", "main", "PR B", "body B")
        .await
        .expect("PR in repo-b");

    // Both should get id 1 (independent counters)
    let pra: TestPrFile =
        serde_yaml::from_str(&tokio::fs::read_to_string(&pr_a).await.unwrap()).unwrap();
    let prb: TestPrFile =
        serde_yaml::from_str(&tokio::fs::read_to_string(&pr_b).await.unwrap()).unwrap();

    assert_eq!(pra.id, 1, "repo-a PR should get id 1");
    assert_eq!(prb.id, 1, "repo-b PR should get id 1");

    // Files should be in separate directories
    assert!(
        pr_a.contains("/repo-a/"),
        "PR A path should contain /repo-a/"
    );
    assert!(
        pr_b.contains("/repo-b/"),
        "PR B path should contain /repo-b/"
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_ensure_branch_and_pr_no_work_dir() {
    let setup = create_test_setup().await;
    let src = source_repo_str(&setup);

    let result = setup
        .backend
        .ensure_branch_and_pr(
            &src,
            Path::new("/tmp/nonexistent_zbobr_test"),
            "feature-branch",
            "main",
            "title",
        )
        .await;

    assert!(result.is_err(), "should fail when work dir does not exist");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Work directory does not exist"),
        "error should mention missing work directory, got: {err}"
    );
}

#[tokio::test]
async fn test_parse_pr_nonexistent_file() {
    let setup = create_test_setup().await;

    let result = setup
        .backend
        .parse_pr_to_repo_branch("/nonexistent/path.yaml")
        .await;

    assert!(result.is_err(), "should fail for nonexistent file");
}

// ---------------------------------------------------------------------------
// End-to-end workflow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_full_workflow() {
    let setup = create_test_setup().await;
    let ws = workspace_path(&setup, "full_workflow");
    let src = source_repo_str(&setup);

    // Step 1: Clone
    let clone_dir = setup
        .backend
        .clone_and_setup(&src, "main", "main", &ws)
        .await
        .expect("clone should succeed");

    // Step 2: Create feature branch with commit
    create_work_branch(&clone_dir, "zbobr-42-feature").await;

    // Step 3: Push the feature branch
    setup
        .backend
        .setup_fork_remote_and_push(&clone_dir, &src, "zbobr-42-feature")
        .await
        .expect("push should succeed");

    // Step 4: Create PR
    let repo_name = Path::new(&src)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("repo name");

    let pr_path = setup
        .backend
        .create_pr_in_fork(
            repo_name,
            "zbobr-42-feature",
            "main",
            "Feature 42",
            "Implements feature #42",
        )
        .await
        .expect("create_pr_in_fork should succeed");

    // Step 5: Parse PR back
    let (repo, branch) = setup
        .backend
        .parse_pr_to_repo_branch(&pr_path)
        .await
        .expect("parse_pr should succeed");

    assert_eq!(repo, repo_name, "parsed repo should match repo name");
    assert_eq!(branch, "zbobr-42-feature");

    // Step 6: Verify branch exists in bare repo
    let branches = git_command(
        &setup.source_repo,
        &["branch", "--list", "zbobr-42-feature"],
    )
    .await;
    assert!(
        branches.contains("zbobr-42-feature"),
        "feature branch should exist in bare repo"
    );
}

// ---------------------------------------------------------------------------
// Worktree operations
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_clone_and_setup_with_worktrees() {
    let setup = create_test_setup_with_worktrees().await;
    let ws = workspace_path(&setup, "worktree_test");
    let src = source_repo_str(&setup);

    let worktree_path = setup
        .backend
        .clone_and_setup(&src, "main", "main", &ws)
        .await
        .expect("clone_and_setup should succeed");

    assert!(worktree_path.join(".git").exists(), ".git should exist in worktree");
    assert!(
        worktree_path.join("README.md").exists(),
        "README.md should exist in worktree"
    );

    let branch = git_command(&worktree_path, &["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert_eq!(branch, "main");
}

#[tokio::test]
async fn test_multiple_worktrees_same_repo() {
    let setup = create_test_setup_with_worktrees().await;
    let src = source_repo_str(&setup);

    let ws1 = workspace_path(&setup, "worktree1");
    let ws2 = workspace_path(&setup, "worktree2");

    let wt1 = setup
        .backend
        .clone_and_setup(&src, "main", "main", &ws1)
        .await
        .expect("first worktree should succeed");

    let wt2 = setup
        .backend
        .clone_and_setup(&src, "feature", "main", &ws2)
        .await
        .expect("second worktree should succeed");

    assert_ne!(wt1, wt2, "worktrees should be in different paths");
    assert!(wt1.exists(), "first worktree should exist");
    assert!(wt2.exists(), "second worktree should exist");

    let branch1 = git_command(&wt1, &["rev-parse", "--abbrev-ref", "HEAD"]).await;
    let branch2 = git_command(&wt2, &["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert_eq!(branch1, "main");
    assert_eq!(branch2, "feature");
}

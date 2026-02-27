/// Shared test bodies used by all four backend-combination files.
/// Each function takes a fully-initialised `IntegrationTestEnv` and runs
/// one complete test scenario against it.
use std::path::PathBuf;
use zbobr_dispatcher::Stage;

use super::env::IntegrationTestEnv;
use super::scenarios;

// ---------------------------------------------------------------------------
// Preparation
// ---------------------------------------------------------------------------

pub async fn run_preparation(env: &IntegrationTestEnv) {
    let repo_path = env.create_git_repo("repo_preparation").await;
    let task_id = env
        .create_task("Dummy Task", "Dummy task description", Stage::Preparing)
        .await;

    env.run_stage(
        task_id,
        Stage::Preparing,
        scenarios::preparation_scenario(&repo_path.to_string_lossy()),
    )
    .await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Signal:      go_plan"),
        "[{}] Preparator should emit go_plan after setting repo/branches",
        env.name()
    );
}

// ---------------------------------------------------------------------------
// Planning
// ---------------------------------------------------------------------------

pub async fn run_planning(env: &IntegrationTestEnv) {
    let repo_path = env.create_git_repo("repo_planning").await;
    let task_id = env
        .create_task("Dummy Task", "Dummy task description", Stage::Preparing)
        .await;

    let dest_repo = env.target_repo
        .as_deref()
        .map(|r| format!("https://github.com/{r}"))
        .unwrap_or_else(|| repo_path.to_string_lossy().to_string());
    let repo_name = dest_repo.rsplit('/').next().unwrap_or(&dest_repo).to_string();
    let work_branch = format!("zbobr_fix-{task_id}-test");
    env.update_task_branches(task_id, &dest_repo, "main", &work_branch).await;

    env.run_stage(task_id, Stage::Planning, scenarios::planning_scenario())
        .await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Signal:      go_work"),
        "[{}] Planner should emit go_work after posting plan",
        env.name()
    );
    assert!(
        output.contains("pr_url:"),
        "[{}] PR URL should be stored after planning stage:\n{output}",
        env.name()
    );
    assert_pr_url_points_to_branch(env, &output, &work_branch).await;

    assert_workspace_ok(env, task_id, &repo_name, &work_branch).await;
}

// ---------------------------------------------------------------------------
// Working
// ---------------------------------------------------------------------------

pub async fn run_working(env: &IntegrationTestEnv) {
    let repo_path = env.create_git_repo("repo_working").await;
    let task_id = env
        .create_task("Dummy Task", "Dummy task description", Stage::Working)
        .await;

    let dest_repo = env.target_repo
        .as_deref()
        .map(|r| format!("https://github.com/{r}"))
        .unwrap_or_else(|| repo_path.to_string_lossy().to_string());
    let repo_name = dest_repo.rsplit('/').next().unwrap_or(&dest_repo).to_string();
    let work_branch = format!("zbobr_fix-{task_id}-test");
    env.update_task_branches(task_id, &dest_repo, "main", &work_branch).await;

    env.run_stage(task_id, Stage::Working, scenarios::working_scenario())
        .await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Worker complete."),
        "[{}] Worker report not found in discussion",
        env.name()
    );
    assert!(
        output.contains("Signal:      go_review"),
        "[{}] Worker should emit go_review when all checklist items are checked",
        env.name()
    );
    assert!(
        output.contains("[x] Implement and validate worker stage integration coverage"),
        "[{}] Expected checked checklist item not found",
        env.name()
    );
    assert!(
        output.contains("pr_url:"),
        "[{}] PR URL should be stored after working stage:\n{output}",
        env.name()
    );
    assert_pr_url_points_to_branch(env, &output, &work_branch).await;

    assert_workspace_ok(env, task_id, &repo_name, &work_branch).await;
}

// ---------------------------------------------------------------------------
// Reviewing
// ---------------------------------------------------------------------------

pub async fn run_reviewing(env: &IntegrationTestEnv) {
    let repo_path = env.create_git_repo("repo_reviewing").await;
    let task_id = env
        .create_task("Dummy Task", "Dummy task description", Stage::Reviewing)
        .await;

    let dest_repo = env.target_repo
        .as_deref()
        .map(|r| format!("https://github.com/{r}"))
        .unwrap_or_else(|| repo_path.to_string_lossy().to_string());
    let repo_name = dest_repo.rsplit('/').next().unwrap_or(&dest_repo).to_string();
    let work_branch = format!("zbobr_fix-{task_id}-test");
    env.update_task_branches(task_id, &dest_repo, "main", &work_branch).await;

    env.run_stage(task_id, Stage::Reviewing, scenarios::reviewing_scenario())
        .await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Reviewer complete."),
        "[{}] Reviewer report not found in discussion",
        env.name()
    );
    assert!(
        output.contains("Signal:      go_work"),
        "[{}] Reviewer should emit go_work when checklist has unchecked items",
        env.name()
    );
    assert!(
        output.contains("[ ] Fix review issue: adjust edge-case handling"),
        "[{}] Expected unchecked review item not found",
        env.name()
    );
    assert!(
        output.contains("pr_url:"),
        "[{}] PR URL should be stored after reviewing stage:\n{output}",
        env.name()
    );
    assert_pr_url_points_to_branch(env, &output, &work_branch).await;

    assert_workspace_ok(env, task_id, &repo_name, &work_branch).await;
}

// ---------------------------------------------------------------------------
// Reviewing — approval path (no issues → DONE + PR)
// ---------------------------------------------------------------------------

pub async fn run_reviewing_approval(env: &IntegrationTestEnv) {
    let repo_path = env.create_git_repo("repo_reviewing_approval").await;
    let task_id = env
        .create_task("Dummy Task", "Dummy task description", Stage::Reviewing)
        .await;

    let dest_repo = env.target_repo
        .as_deref()
        .map(|r| format!("https://github.com/{r}"))
        .unwrap_or_else(|| repo_path.to_string_lossy().to_string());
    let repo_name = dest_repo.rsplit('/').next().unwrap_or(&dest_repo).to_string();
    let work_branch = format!("zbobr_fix-{task_id}-test");
    env.update_task_branches(task_id, &dest_repo, "main", &work_branch).await;

    if let Some(target) = env.target_repo.as_deref() {
        env.prepare_workspace_via_repo_backend(task_id, target, &work_branch).await;
    } else {
        env.prepare_workspace(task_id, &repo_path, &work_branch).await;
    }

    // Add a placeholder commit so the work branch differs from main (PR requires changes).
    let work_dir = env
        .workspaces_dir
        .join(format!("task#{task_id}"))
        .join(&repo_name);
    write_and_commit(
        &work_dir,
        "ZBOBR_PLACEHOLDER.md",
        &format!("placeholder for task #{task_id}\n"),
        "chore: add placeholder for PR",
    )
    .await;

    env.run_stage(
        task_id,
        Stage::Reviewing,
        scenarios::reviewing_approval_scenario(),
    )
    .await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Stage:       DONE"),
        "[{}] Reviewer approval should move task to done:\n{output}",
        env.name()
    );
    assert!(
        output.contains("pr_url:"),
        "[{}] PR URL should be stored after reviewer approval:\n{output}",
        env.name()
    );
    assert_pr_url_points_to_branch(env, &output, &work_branch).await;
    assert_pr_has_commits(env, &output, "main").await;
}

// ---------------------------------------------------------------------------
// Merging
// ---------------------------------------------------------------------------

pub async fn run_merging(env: &IntegrationTestEnv) {
    let repo_path = env.create_git_repo("repo_merging").await;
    let dest_repo = env.target_repo
        .as_deref()
        .map(|r| format!("https://github.com/{r}"))
        .unwrap_or_else(|| repo_path.to_string_lossy().to_string());
    let repo_name = dest_repo.rsplit('/').next().unwrap_or(&dest_repo).to_string();

    // ---- report ending ----
    let task_report = env
        .create_task("Dummy Task", "Dummy task description", Stage::Merging)
        .await;
    let branch_report = format!("zbobr_fix-{task_report}-test");
    env.update_task_branches(task_report, &dest_repo, "main", &branch_report)
        .await;
    if let Some(target) = env.target_repo.as_deref() {
        env.prepare_workspace_via_repo_backend(task_report, target, &branch_report).await;
    } else {
        env.prepare_workspace(task_report, &repo_path, &branch_report).await;
    }

    // Add a dummy commit so the work branch has changes above main (required for PR creation).
    let work_dir_report = env
        .workspaces_dir
        .join(format!("task#{task_report}"))
        .join(&repo_name);
    write_and_commit(
        &work_dir_report,
        "ZBOBR_PLACEHOLDER.md",
        &format!("placeholder for task #{task_report}\n"),
        "chore: add placeholder for PR",
    )
    .await;

    env.run_stage(
        task_report,
        Stage::Merging,
        scenarios::merging_scenario("report"),
    )
    .await;

    let output = env.show_task(task_report).await;
    assert!(
        output.contains("Merger complete."),
        "[{}] Merger report not found in discussion",
        env.name()
    );
    assert!(
        output.contains("Signal:      (none)"),
        "[{}] Merger should not set a follow-up signal",
        env.name()
    );
    // The PR URL is stored at workspace setup time (ensure_pr_url in run_role_session).
    assert!(
        output.contains("pr_url:"),
        "[{}] PR URL should be stored in task parameters after merger:\n{output}",
        env.name()
    );
    assert_pr_url_points_to_branch(env, &output, &branch_report).await;
    assert_pr_has_commits(env, &output, "main").await;

    assert_workspace_ok(env, task_report, &repo_name, &branch_report).await;

    // ---- ask ending ----
    let task_ask = env
        .create_task("Dummy Task", "Dummy task description", Stage::Merging)
        .await;
    let branch_ask = format!("zbobr_fix-{task_ask}-test");
    env.update_task_branches(task_ask, &dest_repo, "main", &branch_ask)
        .await;
    if let Some(target) = env.target_repo.as_deref() {
        env.prepare_workspace_via_repo_backend(task_ask, target, &branch_ask).await;
    } else {
        env.prepare_workspace(task_ask, &repo_path, &branch_ask).await;
    }

    env.run_stage(
        task_ask,
        Stage::Merging,
        scenarios::merging_scenario("ask"),
    )
    .await;

    let output = env.show_task(task_ask).await;
    assert!(
        output.contains("Need guidance on merge"),
        "[{}] Ask-user message not found in discussion",
        env.name()
    );
    assert!(
        output.contains("Pause:       true"),
        "[{}] ask_user should set the pause flag",
        env.name()
    );
}

pub async fn run_merging_with_real_conflict(env: &IntegrationTestEnv) {
    let repo_path = env.create_git_repo("repo_merging_conflict").await;
    let repo_path_str = repo_path.to_string_lossy().to_string();
    let dest_repo = env.target_repo
        .as_deref()
        .map(|r| format!("https://github.com/{r}"))
        .unwrap_or_else(|| repo_path_str.clone());
    let repo_name = dest_repo.rsplit('/').next().unwrap_or(&dest_repo).to_string();

    let task_id = env
        .create_task("Conflict task", "Test merging with real conflicts", Stage::Merging)
        .await;
    let work_branch = format!("zbobr_conflict-{task_id}-test");
    env.update_task_branches(task_id, &dest_repo, "main", &work_branch)
        .await;

    // Set up workspace with a live merge conflict.
    // When a real GitHub repo backend is configured, clone via the backend so
    // that origin/fork remotes are correctly set up (PR creation can succeed).
    // Otherwise fall back to the cp -r approach with a local bare repo.
    let work_dir = if let Some(target) = env.target_repo.as_deref() {
        let wd = env.prepare_workspace_via_repo_backend(task_id, target, &work_branch).await;

        // Inject conflicting changes locally (conflict_file.txt does not exist
        // on the remote, so both branches adding it differently is an add/add conflict).
        write_and_commit(&wd, "conflict_file.txt", "line1\nline2 work\nline3\n", "Work change").await;

        git_in(&wd, &["checkout", "main"]).await;
        write_and_commit(&wd, "conflict_file.txt", "line1\nline2 main\nline3\n", "Main change").await;

        git_in(&wd, &["checkout", &work_branch]).await;
        wd
    } else {
        // Local-only: build conflicting history on the source repo then cp -r.
        write_and_commit(&repo_path, "conflict_file.txt", "line1\nline2\nline3\n", "Initial").await;

        git_in(&repo_path, &["checkout", "-b", &work_branch]).await;
        write_and_commit(&repo_path, "conflict_file.txt", "line1\nline2 work\nline3\n", "Work change").await;

        git_in(&repo_path, &["checkout", "main"]).await;
        write_and_commit(&repo_path, "conflict_file.txt", "line1\nline2 main\nline3\n", "Main change").await;

        let workspace_dir = env.workspaces_dir.join(format!("task#{task_id}"));
        tokio::fs::create_dir_all(&workspace_dir).await.unwrap();
        let wd = workspace_dir.join(&repo_name);

        let cp_ok = tokio::process::Command::new("cp")
            .args(["-r", &repo_path_str, wd.to_str().unwrap()])
            .status()
            .await
            .unwrap()
            .success();
        assert!(cp_ok, "[{}] cp to workspace failed", env.name());

        git_in(&wd, &["checkout", &work_branch]).await;
        wd
    };

    // merge should produce conflict markers
    let merge = tokio::process::Command::new("git")
        .args(["merge", "main", "--no-edit"])
        .current_dir(&work_dir)
        .output()
        .await
        .unwrap();
    assert!(
        !merge.status.success(),
        "[{}] Expected merge conflict but merge succeeded",
        env.name()
    );

    env.run_stage(
        task_id,
        Stage::Merging,
        scenarios::merging_conflict_scenario(),
    )
    .await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Detected merge conflicts"),
        "[{}] Merger should report detected conflicts",
        env.name()
    );
    assert!(
        output.contains("Conflict:    false"),
        "[{}] Merger should clear the conflict flag",
        env.name()
    );
}

// ---------------------------------------------------------------------------
// Conflict detection
// ---------------------------------------------------------------------------

/// Verify the automatic conflict-detection path in `run_role_session`:
///
/// 1. The dispatcher clones the work branch.
/// 2. It tries `git merge <dest_branch>` and finds a conflict.
/// 3. It sets `conflict = true`, reverts the task to Pending, and returns
///    without launching the agent.
/// 4. The Merger is then run on the already-conflicted workspace and clears
///    the flag.
///
/// This test relies on a purely local git repo and is skipped when a GitHub
/// repo backend is configured (`env.target_repo.is_some()`).
pub async fn run_conflict_detection(env: &IntegrationTestEnv) {
    if env.target_repo.is_some() {
        eprintln!(
            "[{}] Skipping run_conflict_detection: requires local repo backend",
            env.name()
        );
        return;
    }

    let repo_path = env.create_git_repo("repo_conflict_detection").await;
    let repo_path_str = repo_path.to_string_lossy().to_string();
    let repo_name = repo_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let work_branch = "zbobr_conflict-detect-work";

    // Build conflicting histories: both main and work_branch add different
    // content to conflict_file.txt after their common ancestor.
    git_in(&repo_path, &["checkout", "-b", work_branch]).await;
    write_and_commit(
        &repo_path,
        "conflict_file.txt",
        "line1\nline2 work\nline3\n",
        "Work change",
    )
    .await;
    git_in(&repo_path, &["checkout", "main"]).await;
    write_and_commit(
        &repo_path,
        "conflict_file.txt",
        "line1\nline2 main\nline3\n",
        "Main change",
    )
    .await;

    let task_id = env
        .create_task(
            "Conflict Detection",
            "Dummy task description",
            Stage::Working,
        )
        .await;
    env.update_task_branches(task_id, &repo_path_str, "main", work_branch)
        .await;

    // Run the Worker stage.  The dispatcher clones the repo, attempts
    // `git merge main`, detects the conflict, sets conflict=true, and exits
    // successfully without invoking the mcp-tester agent.
    env.run_stage(task_id, Stage::Working, scenarios::working_scenario())
        .await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Conflict:    true"),
        "[{}] Conflict flag should be set after automatic conflict detection:\n{output}",
        env.name()
    );

    // Workspace must exist and be in an unresolved merge state.
    let work_dir = env
        .workspaces_dir
        .join(format!("task#{task_id}"))
        .join(&repo_name);
    let git_status = git_output(&work_dir, &["status"]).await;
    assert!(
        git_status.contains("You have unmerged paths")
            || git_status.contains("Unmerged paths"),
        "[{}] Workspace should be in a conflicted git state:\n{git_status}",
        env.name()
    );

    // Run the Merger on the already-conflicted workspace.
    env.run_stage(
        task_id,
        Stage::Merging,
        scenarios::merging_conflict_scenario(),
    )
    .await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Conflict:    false"),
        "[{}] Conflict flag should be cleared after Merger session:\n{output}",
        env.name()
    );
}

// ---------------------------------------------------------------------------
// Repo backend — same-org clone
// ---------------------------------------------------------------------------

/// A well-known public repository owned by a different organisation than any
/// typical test user.  Used by the cross-org tests to exercise the fork
/// creation path (fork_owner != target repo owner).
const CROSS_ORG_DEST_REPO: &str = "octocat/Spoon-Knife";

/// Test `clone_and_setup` against a same-org target (`env.target_repo`).
/// Verifies the workspace is cloned and that no "fork" remote is created
/// (same-org mode: `fork_owner` == target repo owner).
///
/// Skipped when the repo backend is not GitHub or `target_repo` is not set.
pub async fn run_repo_backend_clone(env: &IntegrationTestEnv) {
    let Some(target) = env.target_repo.as_deref() else {
        eprintln!(
            "[{}] Skipping run_repo_backend_clone: target_repo not configured",
            env.name()
        );
        return;
    };
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_clone: not a GitHub repo backend",
            env.name()
        );
        return;
    }

    let repo_name = target.rsplit('/').next().unwrap_or(target);
    let task_id = env
        .create_task("Clone test", "Test clone_and_setup via repo backend", Stage::Pending)
        .await;
    let work_branch = format!("zbobr_fix-{task_id}-clone-test");
    env.update_task_branches(task_id, target, "main", &work_branch).await;

    env.run_zbobr("task", &["clone", &task_id.to_string()]).await;

    let workspace_dir = env.workspaces_dir.join(format!("task#{task_id}")).join(repo_name);
    assert!(
        workspace_dir.exists(),
        "[{}] Workspace directory missing after clone: {}",
        env.name(),
        workspace_dir.display()
    );
    assert!(
        workspace_dir.join(".git").exists(),
        "[{}] Workspace is not a git repository",
        env.name()
    );

    let origin_url = tokio::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&workspace_dir)
        .output()
        .await
        .expect("failed to run git remote get-url origin");
    assert!(
        origin_url.status.success(),
        "[{}] origin remote not found in workspace",
        env.name()
    );
    let origin = String::from_utf8_lossy(&origin_url.stdout);
    assert!(
        origin.contains(repo_name),
        "[{}] origin remote '{}' does not contain repo name '{}'",
        env.name(),
        origin.trim(),
        repo_name
    );

    // Same-org mode: no fork remote expected
    let fork_check = tokio::process::Command::new("git")
        .args(["remote", "get-url", "fork"])
        .current_dir(&workspace_dir)
        .output()
        .await
        .expect("failed to run git remote get-url fork");
    assert!(
        !fork_check.status.success(),
        "[{}] Unexpected fork remote found in same-org workspace",
        env.name()
    );
}

/// Test `clone_and_setup` against `octocat/Spoon-Knife` (cross-org).
/// Verifies the workspace is cloned and that a "fork" remote IS created
/// (cross-org mode: `fork_owner` != `octocat`).
///
/// Skipped when the repo backend is not GitHub.
pub async fn run_repo_backend_clone_cross_org(env: &IntegrationTestEnv) {
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_clone_cross_org: not a GitHub repo backend",
            env.name()
        );
        return;
    }

    let target = CROSS_ORG_DEST_REPO;
    let repo_name = target.rsplit('/').next().unwrap_or(target);

    let task_id = env
        .create_task(
            "Clone cross-org test",
            "Test clone_and_setup with cross-org repo (octocat/Spoon-Knife)",
            Stage::Pending,
        )
        .await;
    let work_branch = format!("zbobr_fix-{task_id}-clone-xorg");
    env.update_task_branches(task_id, target, "main", &work_branch).await;

    env.run_zbobr("task", &["clone", &task_id.to_string()]).await;

    let workspace_dir = env.workspaces_dir.join(format!("task#{task_id}")).join(repo_name);
    assert!(
        workspace_dir.exists(),
        "[{}] Workspace directory missing after cross-org clone: {}",
        env.name(),
        workspace_dir.display()
    );
    assert!(
        workspace_dir.join(".git").exists(),
        "[{}] Workspace is not a git repository after cross-org clone",
        env.name()
    );

    let origin_out = tokio::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&workspace_dir)
        .output()
        .await
        .expect("failed to run git remote get-url origin");
    assert!(
        origin_out.status.success(),
        "[{}] origin remote not found in cross-org workspace",
        env.name()
    );
    let origin = String::from_utf8_lossy(&origin_out.stdout);
    assert!(
        origin.contains(repo_name),
        "[{}] origin remote '{}' does not contain repo name '{}'",
        env.name(),
        origin.trim(),
        repo_name
    );

    // Cross-org mode: fork remote must exist
    let fork_out = tokio::process::Command::new("git")
        .args(["remote", "get-url", "fork"])
        .current_dir(&workspace_dir)
        .output()
        .await
        .expect("failed to run git remote get-url fork");
    assert!(
        fork_out.status.success(),
        "[{}] Fork remote missing in cross-org workspace (expected fork of {} under {})",
        env.name(),
        target,
        env.fork_owner().unwrap_or("<unknown>")
    );
    let fork_url = String::from_utf8_lossy(&fork_out.stdout);
    if let Some(fork_owner) = env.fork_owner() {
        assert!(
            fork_url.contains(fork_owner),
            "[{}] fork remote '{}' does not contain fork owner '{}'",
            env.name(),
            fork_url.trim(),
            fork_owner
        );
    }
}

// ---------------------------------------------------------------------------
// Repo backend — same-org planning / working / reviewing / merging
// ---------------------------------------------------------------------------

/// Run the Planning stage against `env.target_repo` via the GitHub repo backend.
/// Skipped when the repo backend is not GitHub or `target_repo` is not set.
pub async fn run_repo_backend_planning(env: &IntegrationTestEnv) {
    let Some(target) = env.target_repo.as_deref() else {
        eprintln!(
            "[{}] Skipping run_repo_backend_planning: target_repo not configured",
            env.name()
        );
        return;
    };
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_planning: not a GitHub repo backend",
            env.name()
        );
        return;
    }
    repo_backend_planning_for(env, target, "plan").await;
}

/// Run the Working stage against `env.target_repo` via the GitHub repo backend.
/// Skipped when the repo backend is not GitHub or `target_repo` is not set.
pub async fn run_repo_backend_working(env: &IntegrationTestEnv) {
    let Some(target) = env.target_repo.as_deref() else {
        eprintln!(
            "[{}] Skipping run_repo_backend_working: target_repo not configured",
            env.name()
        );
        return;
    };
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_working: not a GitHub repo backend",
            env.name()
        );
        return;
    }
    repo_backend_working_for(env, target, "work").await;
}

/// Run the Reviewing stage against `env.target_repo` via the GitHub repo backend.
/// Skipped when the repo backend is not GitHub or `target_repo` is not set.
pub async fn run_repo_backend_reviewing(env: &IntegrationTestEnv) {
    let Some(target) = env.target_repo.as_deref() else {
        eprintln!(
            "[{}] Skipping run_repo_backend_reviewing: target_repo not configured",
            env.name()
        );
        return;
    };
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_reviewing: not a GitHub repo backend",
            env.name()
        );
        return;
    }
    repo_backend_reviewing_for(env, target, "review").await;
}

/// Run the Merging stage against `env.target_repo` via the GitHub repo backend.
/// Skipped when the repo backend is not GitHub or `target_repo` is not set.
pub async fn run_repo_backend_merging(env: &IntegrationTestEnv) {
    let Some(target) = env.target_repo.as_deref() else {
        eprintln!(
            "[{}] Skipping run_repo_backend_merging: target_repo not configured",
            env.name()
        );
        return;
    };
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_merging: not a GitHub repo backend",
            env.name()
        );
        return;
    }
    repo_backend_merging_for(env, target, "merge").await;
}

// ---------------------------------------------------------------------------
// Repo backend — cross-org planning / working / reviewing / merging
// ---------------------------------------------------------------------------

/// Run the Planning stage against `octocat/Spoon-Knife` (cross-org fork path).
/// Skipped when the repo backend is not GitHub.
pub async fn run_repo_backend_planning_cross_org(env: &IntegrationTestEnv) {
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_planning_cross_org: not a GitHub repo backend",
            env.name()
        );
        return;
    }
    repo_backend_planning_for(env, CROSS_ORG_DEST_REPO, "xorg-plan").await;
}

/// Run the Working stage against `octocat/Spoon-Knife` (cross-org fork path).
/// Skipped when the repo backend is not GitHub.
pub async fn run_repo_backend_working_cross_org(env: &IntegrationTestEnv) {
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_working_cross_org: not a GitHub repo backend",
            env.name()
        );
        return;
    }
    repo_backend_working_for(env, CROSS_ORG_DEST_REPO, "xorg-work").await;
}

/// Run the Reviewing stage against `octocat/Spoon-Knife` (cross-org fork path).
/// Skipped when the repo backend is not GitHub.
pub async fn run_repo_backend_reviewing_cross_org(env: &IntegrationTestEnv) {
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_reviewing_cross_org: not a GitHub repo backend",
            env.name()
        );
        return;
    }
    repo_backend_reviewing_for(env, CROSS_ORG_DEST_REPO, "xorg-review").await;
}

/// Run the Merging stage against `octocat/Spoon-Knife` (cross-org fork path).
/// Skipped when the repo backend is not GitHub.
pub async fn run_repo_backend_merging_cross_org(env: &IntegrationTestEnv) {
    if env.fork_owner().is_none() {
        eprintln!(
            "[{}] Skipping run_repo_backend_merging_cross_org: not a GitHub repo backend",
            env.name()
        );
        return;
    }
    repo_backend_merging_for(env, CROSS_ORG_DEST_REPO, "xorg-merge").await;
}

// ---------------------------------------------------------------------------
// Confirm flag
// ---------------------------------------------------------------------------

/// Verify that `--confirm` causes an automatic pause on stage transition.
pub async fn run_cli_confirm_flag(env: &IntegrationTestEnv) {
    let task_id = env
        .create_task_with_confirm("Confirm test", "desc", Stage::Pending, true)
        .await;

    env.run_zbobr(
        "task",
        &["update", &task_id.to_string(), "--stage", "PLANNING"],
    )
    .await;
    let output2 = env.show_task(task_id).await;
    assert!(
        output2.contains("Pause:       true"),
        "[{}] task should be paused after stage change with confirm\n{output2}",
        env.name()
    );
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Extract the `pr_url` value from `zbobr task show` output.
fn extract_pr_url(output: &str) -> Option<String> {
    output
        .lines()
        .find(|l| l.trim_start().starts_with("pr_url:"))
        .and_then(|l| l.splitn(2, ':').nth(1))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Validate that the PR reference stored in the task output is correct:
/// - The path exists as a git repository.
/// - The currently checked-out branch matches `expected_branch`.
///
/// For the FS backend, `pr_url` is the work directory path, so these checks
/// exercise the full chain: workspace setup → ensure_branch_and_pr → stored URL.
async fn assert_pr_url_points_to_branch(env: &IntegrationTestEnv, output: &str, expected_branch: &str) {
    let pr_url = extract_pr_url(output).unwrap_or_else(|| {
        panic!(
            "[{}] pr_url not found in task output:\n{output}",
            env.name()
        )
    });

    // For the GitHub backend the pr_url is an https:// URL — skip git checks.
    if pr_url.starts_with("http") {
        return;
    }

    let pr_path = PathBuf::from(&pr_url);
    assert!(
        pr_path.join(".git").exists(),
        "[{}] pr_url '{}' is not a git repository",
        env.name(),
        pr_url
    );

    let current = git_output(&pr_path, &["branch", "--show-current"]).await;
    assert_eq!(
        current.trim(),
        expected_branch,
        "[{}] pr_url '{}' is not on the expected branch",
        env.name(),
        pr_url
    );
}

/// Verify the work branch in the pr_url directory has at least one commit
/// ahead of `origin/main` (i.e., real work was pushed into the PR).
async fn assert_pr_has_commits(env: &IntegrationTestEnv, output: &str, dest_branch: &str) {
    let pr_url = match extract_pr_url(output) {
        Some(u) => u,
        None => return, // already asserted elsewhere
    };

    if pr_url.starts_with("http") {
        // GitHub backend: verify the PR via the API.
        assert_github_pr_has_commits(env, &pr_url, dest_branch).await;
        return;
    }

    let pr_path = PathBuf::from(&pr_url);
    let log = git_output(
        &pr_path,
        &["log", &format!("origin/{}..HEAD", dest_branch), "--oneline"],
    )
    .await;

    assert!(
        !log.trim().is_empty(),
        "[{}] pr_url '{}' work branch has no commits ahead of origin/{} — expected at least one",
        env.name(),
        pr_url,
        dest_branch
    );
}

/// Parse a GitHub PR URL (`https://github.com/{owner}/{repo}/pull/{number}`)
/// and use `gh api` to verify the PR exists, has at least one commit, and
/// targets `dest_branch`.
async fn assert_github_pr_has_commits(env: &IntegrationTestEnv, pr_url: &str, dest_branch: &str) {
    // Split: ["https:", "", "github.com", owner, repo, "pull", number, ...]
    let parts: Vec<&str> = pr_url.trim_end_matches('/').splitn(8, '/').collect();
    assert!(
        parts.len() >= 7 && parts[5] == "pull",
        "[{}] Cannot parse GitHub PR URL: {pr_url}",
        env.name()
    );
    let (owner, repo, pr_number) = (parts[3], parts[4], parts[6]);

    let api_path = format!("repos/{owner}/{repo}/pulls/{pr_number}");
    let out = tokio::process::Command::new("gh")
        .args(["api", &api_path])
        .output()
        .await
        .unwrap_or_else(|e| panic!("[{}] failed to run `gh api`: {e}", env.name()));

    assert!(
        out.status.success(),
        "[{}] `gh api {api_path}` failed:\n{}",
        env.name(),
        String::from_utf8_lossy(&out.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("[{}] failed to parse gh api response: {e}", env.name()));

    let commits = json["commits"].as_u64().unwrap_or(0);
    assert!(
        commits > 0,
        "[{}] GitHub PR {pr_url} has 0 commits — expected at least one",
        env.name()
    );

    let base_ref = json["base"]["ref"].as_str().unwrap_or("unknown");
    assert_eq!(
        base_ref, dest_branch,
        "[{}] GitHub PR {pr_url} targets branch '{}', expected '{dest_branch}'",
        env.name(),
        base_ref
    );
}

async fn repo_backend_planning_for(env: &IntegrationTestEnv, target: &str, suffix: &str) {
    let task_id = env
        .create_task("Repo backend planning", "Dummy task description", Stage::Pending)
        .await;
    let work_branch = format!("zbobr_fix-{task_id}-{suffix}-test");
    env.update_task_branches(task_id, target, "main", &work_branch).await;
    env.prepare_workspace_via_repo_backend(task_id, target, &work_branch).await;

    env.run_stage(task_id, Stage::Planning, scenarios::planning_scenario()).await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Signal:      go_work"),
        "[{}] Planner should emit go_work after posting plan",
        env.name()
    );
}

async fn repo_backend_working_for(env: &IntegrationTestEnv, target: &str, suffix: &str) {
    let task_id = env
        .create_task("Repo backend working", "Dummy task description", Stage::Working)
        .await;
    let work_branch = format!("zbobr_fix-{task_id}-{suffix}-test");
    env.update_task_branches(task_id, target, "main", &work_branch).await;
    env.prepare_workspace_via_repo_backend(task_id, target, &work_branch).await;

    env.run_stage(task_id, Stage::Working, scenarios::working_scenario()).await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Signal:      go_review"),
        "[{}] Worker should emit go_review",
        env.name()
    );
}

async fn repo_backend_reviewing_for(env: &IntegrationTestEnv, target: &str, suffix: &str) {
    let task_id = env
        .create_task("Repo backend reviewing", "Dummy task description", Stage::Reviewing)
        .await;
    let work_branch = format!("zbobr_fix-{task_id}-{suffix}-test");
    env.update_task_branches(task_id, target, "main", &work_branch).await;
    env.prepare_workspace_via_repo_backend(task_id, target, &work_branch).await;

    env.run_stage(task_id, Stage::Reviewing, scenarios::reviewing_scenario()).await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Signal:      go_work"),
        "[{}] Reviewer should emit go_work when checklist has unchecked items",
        env.name()
    );
}

async fn repo_backend_merging_for(env: &IntegrationTestEnv, target: &str, suffix: &str) {
    let task_id = env
        .create_task("Repo backend merging", "Dummy task description", Stage::Merging)
        .await;
    let work_branch = format!("zbobr_fix-{task_id}-{suffix}-test");
    env.update_task_branches(task_id, target, "main", &work_branch).await;
    env.prepare_workspace_via_repo_backend(task_id, target, &work_branch).await;

    env.run_stage(task_id, Stage::Merging, scenarios::merging_scenario("report")).await;

    let output = env.show_task(task_id).await;
    assert!(
        output.contains("Merger complete."),
        "[{}] Merger report not found in discussion",
        env.name()
    );
}

async fn write_and_commit(repo: &PathBuf, file: &str, content: &str, msg: &str) {
    tokio::fs::write(repo.join(file), content).await.unwrap();
    git_in(repo, &["add", file]).await;
    git_in(repo, &["commit", "-m", msg]).await;
}

async fn git_in(dir: &PathBuf, args: &[&str]) {
    let ok = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .await
        .unwrap()
        .success();
    assert!(ok, "git {:?} in {} failed", args, dir.display());
}

/// Assert that the task workspace exists, is a git repo, contains the right
/// branches, and is currently on the work branch.
async fn assert_workspace_ok(
    env: &IntegrationTestEnv,
    task_id: u64,
    repo_name: &str,
    work_branch: &str,
) {
    let work_dir = env
        .workspaces_dir
        .join(format!("task#{task_id}"))
        .join(repo_name);

    assert!(
        work_dir.exists(),
        "[{}] Workspace directory missing: {}",
        env.name(),
        work_dir.display()
    );
    assert!(
        work_dir.join(".git").exists(),
        "[{}] Workspace is not a git repository",
        env.name()
    );

    let branches = git_output(&work_dir, &["branch"]).await;
    assert!(
        branches.contains("main"),
        "[{}] 'main' branch not found in workspace",
        env.name()
    );
    assert!(
        branches.contains(work_branch),
        "[{}] Work branch '{work_branch}' not found in workspace",
        env.name()
    );

    let current = git_output(&work_dir, &["branch", "--show-current"]).await;
    assert_eq!(
        current.trim(),
        work_branch,
        "[{}] Current branch is not the work branch",
        env.name()
    );
}

async fn git_output(dir: &PathBuf, args: &[&str]) -> String {
    let out = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

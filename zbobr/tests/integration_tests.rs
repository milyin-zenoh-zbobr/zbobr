use zbobr_lib::config::BackendType;
use zbobr_lib::{Stage, Zbobr, ZbobrConfig};

#[tokio::test]
async fn test_full_development_lifecycle() -> anyhow::Result<()> {
    // 1. Setup workspace and config
    let workspace = tempfile::tempdir()?.into_path();
    let config = ZbobrConfig {
        domain_repo: "test/domain".to_string(),
        fork_owner: "test-forks".to_string(),
        workspace: workspace.clone(),
        github_token: "stub-token".to_string(),
        backend: BackendType::Stub,
        default_model: "stub-model".to_string(),
        cli_tool: zbobr_lib::config::CliTool::Stub,
    };

    let zbobr = Zbobr::new(config)?;

    // 2. Create an issue
    let issue_id = zbobr
        .create_issue("Implement feature X", "Feature X description")
        .await?;
    println!("Created issue #{}", issue_id);

    // Verify initial state
    let task = zbobr.get_issue(issue_id).await?;
    assert_eq!(task.stage, Stage::Pending);

    // 3. Move to PLANNING_READY
    zbobr.set_task_stage(issue_id, Stage::PlanningReady).await?;

    // 4. Simulate Planner Session
    // Instead of using the MCP server which is hard to test via HTTP,
    // we use the Session objects directly to verify logic.
    let planner = zbobr.planner_session(issue_id);

    let plan = "Detailed plan for feature X";
    planner.set_plan(plan).await?;
    planner
        .post_message("Planner: requirements analyzed and plan created.")
        .await?;

    // Verify plan set
    let updated_task = zbobr.get_issue(issue_id).await?;
    assert_eq!(updated_task.description, plan);

    // Transitions after planner session
    zbobr.set_task_stage(issue_id, Stage::Pending).await?;

    // 5. Move to WORKING_READY
    zbobr.set_task_stage(issue_id, Stage::WorkingReady).await?;

    // Simulate Worker Session
    let worker = zbobr.worker_session(issue_id);

    // In stub mode, request_repo just returns a path
    let _path = worker.request_repo("test/domain").await?;

    // In stub mode, submit_work returns a fake URL
    let pr_url = worker.submit_work("test/domain").await?;
    assert!(pr_url.contains(&format!("https://github.com/stub/repo/pull/{}", issue_id)));

    worker.mark_done().await?;

    // Verify done
    let task = zbobr.get_issue(issue_id).await?;
    assert!(task.done);
    assert_eq!(task.stage, Stage::Pending);

    // 6. Close issue
    zbobr.close_issue(issue_id).await?;
    let is_closed = zbobr.is_issue_closed(issue_id).await?;
    assert!(is_closed);

    println!("Full development lifecycle verified successfully.");
    Ok(())
}

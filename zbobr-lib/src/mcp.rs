use std::sync::Arc;

use crate::task::Stage;
use crate::Zbobr;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};

// -- Parameter types --

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct PlanParam {
    #[schemars(description = "The plan text")]
    pub plan: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct MessageParam {
    #[schemars(description = "The message to post")]
    pub message: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct RepoParam {
    #[schemars(description = "Target repository in owner/name format")]
    pub repo: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct IssueIdParam {
    #[schemars(description = "The task/issue ID")]
    pub id: u64,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct CreateIssueParam {
    #[schemars(description = "Issue title")]
    pub title: String,
    #[schemars(description = "Issue body")]
    pub body: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct UpdateIssueParam {
    pub id: u64,
    pub body: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct SetStageParam {
    pub id: u64,
    pub stage: Stage,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct LabelParam {
    pub id: u64,
    pub label: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct MilestoneParam {
    #[schemars(description = "Milestone name (e.g. PENDING, PLANNING_READY, etc.)")]
    pub milestone: String,
}

macro_rules! mcp_tools {
    ($mod_name:ident, $($name:ident = $val:expr),* $(,)?) => {
        pub mod $mod_name {
            $(pub const $name: &str = $val;)*
            pub const ALL_TOOLS: &[&str] = &[$($val),*];
        }
    }
}

mcp_tools! {
    planner_tools,
    GET_PLAN = "get_plan",
    SET_PLAN = "set_plan",
    GET_DISCUSSION = "get_discussion",
    POST_MESSAGE = "post_message",
    REQUEST_REPO = "request_repo",
}

mcp_tools! {
    worker_tools,
    GET_PLAN = "get_plan",
    GET_DISCUSSION = "get_discussion",
    POST_MESSAGE = "post_message",
    REQUEST_REPO = "request_repo",
    SUBMIT_WORK = "submit_work",
    MARK_DONE = "mark_done",
}

mcp_tools! {
    admin_tools,
    LIST_ISSUES = "list_issues",
    CREATE_ISSUE = "create_issue",
    GET_ISSUE = "get_issue",
    UPDATE_ISSUE_BODY = "update_issue_body",
    SET_ISSUE_STAGE = "set_issue_stage",
    ADD_ISSUE_LABEL = "add_issue_label",
    REMOVE_ISSUE_LABEL = "remove_issue_label",
    GET_DISCUSSION = "get_discussion",
    DEBUG_STATE = "debug_state",
}

// -- Planner MCP service --

#[derive(Clone)]
pub struct PlannerMcp {
    zbobr: Zbobr,
    task_id: u64,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl PlannerMcp {
    pub fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self {
            zbobr,
            task_id,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get the current plan text for this task")]
    async fn get_plan(&self) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.get_plan().await {
            Ok(plan) => plan,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Update the plan text for this task")]
    async fn set_plan(&self, Parameters(params): Parameters<PlanParam>) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.set_plan(&params.plan).await {
            Ok(()) => "Plan updated successfully".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get all discussion messages on this task")]
    async fn get_discussion(&self) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.get_discussion().await {
            Ok(msgs) => {
                if msgs.is_empty() {
                    "No messages yet.".to_string()
                } else {
                    msgs.join("\n\n---\n\n")
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Post a message to the task discussion")]
    async fn post_message(&self, Parameters(params): Parameters<MessageParam>) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.post_message(&params.message).await {
            Ok(()) => "Message posted".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Clone a repository for investigation (read-only). Returns the local path."
    )]
    async fn request_repo(&self, Parameters(params): Parameters<RepoParam>) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.request_repo(&params.repo).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }
}

#[tool_handler]
impl ServerHandler for PlannerMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "Planner tools: investigate task and create implementation plan.".to_string(),
            ),
            ..Default::default()
        }
    }
}

// -- Worker MCP service --

#[derive(Clone)]
pub struct WorkerMcp {
    zbobr: Zbobr,
    task_id: u64,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl WorkerMcp {
    pub fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self {
            zbobr,
            task_id,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get the current plan text for this task")]
    async fn get_plan(&self) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.get_plan().await {
            Ok(plan) => plan,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get all discussion messages on this task")]
    async fn get_discussion(&self) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.get_discussion().await {
            Ok(msgs) => {
                if msgs.is_empty() {
                    "No messages yet.".to_string()
                } else {
                    msgs.join("\n\n---\n\n")
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Post a message to the task discussion")]
    async fn post_message(&self, Parameters(params): Parameters<MessageParam>) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.post_message(&params.message).await {
            Ok(()) => "Message posted".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Fork and clone a repository for implementation. Returns the local path with feature branch ready."
    )]
    async fn request_repo(&self, Parameters(params): Parameters<RepoParam>) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.request_repo(&params.repo).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Push changes and create a PR from the feature branch. Takes the target repo (owner/name)."
    )]
    async fn submit_work(&self, Parameters(params): Parameters<RepoParam>) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.submit_work(&params.repo).await {
            Ok(pr_url) => format!("PR created: {pr_url}"),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Mark this task as done")]
    async fn mark_done(&self) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.mark_done().await {
            Ok(()) => "Task marked as done".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }
}

#[tool_handler]
impl ServerHandler for WorkerMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "Worker tools: implement task according to plan, submit work, mark done."
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

// -- Admin MCP service --

#[derive(Clone)]
pub struct AdminMcp {
    zbobr: Zbobr,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AdminMcp {
    pub fn new(zbobr: Zbobr) -> Self {
        Self {
            zbobr,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List issues in a specific milestone")]
    async fn list_issues(&self, Parameters(params): Parameters<MilestoneParam>) -> String {
        match self.zbobr.find_tasks_by_stage_name(&params.milestone).await {
            Ok(tasks) => {
                if tasks.is_empty() {
                    "No tasks found.".to_string()
                } else {
                    let lines: Vec<_> = tasks
                        .into_iter()
                        .map(|t| format!("#{} ({}): {}", t.id, t.stage, t.title))
                        .collect();
                    lines.join("\n")
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Create a new issue")]
    async fn create_issue(&self, Parameters(params): Parameters<CreateIssueParam>) -> String {
        match self.zbobr.create_issue(&params.title, &params.body).await {
            Ok(id) => format!("Created issue #{}", id),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get details of an issue")]
    async fn get_issue(&self, Parameters(params): Parameters<IssueIdParam>) -> String {
        match self.zbobr.get_issue(params.id).await {
            Ok(task) => format!(
                "ID: {}\nTitle: {}\nStage: {:?}\nDone: {}\nModel: {:?}\n\n{}",
                task.id, task.title, task.stage, task.done, task.model, task.description
            ),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Update the description of a task")]
    async fn update_issue_body(&self, Parameters(params): Parameters<UpdateIssueParam>) -> String {
        match self.zbobr.update_issue_body(params.id, &params.body).await {
            Ok(()) => "Issue body updated".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Change the stage (milestone) of a task")]
    async fn set_issue_stage(&self, Parameters(params): Parameters<SetStageParam>) -> String {
        match self
            .zbobr
            .set_issue_milestone(params.id, params.stage.milestone_name())
            .await
        {
            Ok(()) => format!("Stage updated to {}", params.stage),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Add a label to an issue")]
    async fn add_issue_label(&self, Parameters(params): Parameters<LabelParam>) -> String {
        match self.zbobr.add_issue_label(params.id, &params.label).await {
            Ok(()) => format!("Label '{}' added", params.label),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Remove a label from an issue")]
    async fn remove_issue_label(&self, Parameters(params): Parameters<LabelParam>) -> String {
        match self
            .zbobr
            .remove_issue_label(params.id, &params.label)
            .await
        {
            Ok(()) => format!("Label '{}' removed", params.label),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get all logs/comments for a task")]
    async fn get_discussion(&self, Parameters(params): Parameters<IssueIdParam>) -> String {
        match self.zbobr.get_issue_comments(params.id).await {
            Ok(comments) => {
                if comments.is_empty() {
                    "No logs/comments yet.".to_string()
                } else {
                    comments.join("\n\n---\n\n")
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "DEBUG: Return internal backend state")]
    async fn debug_state(&self) -> String {
        self.zbobr.debug_state()
    }
}

#[tool_handler]
impl ServerHandler for AdminMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "Admin tools: full control over issues, statuses, and logs.".to_string(),
            ),
            ..Default::default()
        }
    }
}

/// Run the MCP HTTP server scoped to a specific role and task.
pub async fn run_mcp_server(
    zbobr: Zbobr,
    port: u16,
    role: String,
    task_id: Option<u64>,
) -> Result<(), crate::ZbobrError> {
    let zbobr = Arc::new(zbobr);
    let path = if let Some(id) = task_id {
        format!("/{role}/{id}")
    } else {
        format!("/{role}")
    };

    let z = zbobr.clone();
    let router = match role.as_str() {
        "planner" => {
            let id = task_id
                .ok_or_else(|| crate::ZbobrError::Other("Planner needs task_id".to_string()))?;
            let svc = StreamableHttpService::new(
                move || Ok(PlannerMcp::new((*z).clone(), id)),
                Arc::new(LocalSessionManager::default()),
                Default::default(),
            );
            axum::Router::new().nest_service(&path, svc)
        }
        "worker" => {
            let id = task_id
                .ok_or_else(|| crate::ZbobrError::Other("Worker needs task_id".to_string()))?;
            let svc = StreamableHttpService::new(
                move || Ok(WorkerMcp::new((*z).clone(), id)),
                Arc::new(LocalSessionManager::default()),
                Default::default(),
            );
            axum::Router::new().nest_service(&path, svc)
        }
        "admin" => {
            let svc = StreamableHttpService::new(
                move || Ok(AdminMcp::new((*z).clone())),
                Arc::new(LocalSessionManager::default()),
                Default::default(),
            );
            axum::Router::new().nest_service(&path, svc)
        }
        _ => return Err(crate::ZbobrError::Other(format!("Unknown role: {role}"))),
    };

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    tracing::info!("MCP server listening on http://127.0.0.1:{port}{path}");

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await
        .map_err(|e| crate::ZbobrError::Other(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that all tools registered in AdminMcp match the constants in admin_tools.
    #[tokio::test]
    async fn test_admin_tools_consistency() {
        let config = crate::config::ZbobrConfig {
            domain_repo: "test/repo".to_string(),
            fork_owner: "test-owner".to_string(),
            default_model: "test-model".to_string(),
            workspace: std::path::PathBuf::from("/tmp"),
            github_token: "test-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: crate::config::CliTool::Stub,
        };
        let zbobr = Zbobr::new(config).unwrap();
        let admin = AdminMcp::new(zbobr);

        let tools = admin.tool_router.list_all();
        let mut tool_names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        tool_names.sort();

        let mut expected_names = admin_tools::ALL_TOOLS.to_vec();
        expected_names.sort();

        assert_eq!(
            tool_names, expected_names,
            "Exposed admin tools do not match admin_tools::ALL_TOOLS"
        );
    }

    #[tokio::test]
    async fn test_planner_tools_consistency() {
        let config = crate::config::ZbobrConfig {
            domain_repo: "test/repo".to_string(),
            fork_owner: "test-owner".to_string(),
            default_model: "test-model".to_string(),
            workspace: std::path::PathBuf::from("/tmp"),
            github_token: "test-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: crate::config::CliTool::Stub,
        };
        let zbobr = Zbobr::new(config).unwrap();
        let planner = PlannerMcp::new(zbobr, 123);

        let tools = planner.tool_router.list_all();
        let mut tool_names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        tool_names.sort();

        let mut expected_names = planner_tools::ALL_TOOLS.to_vec();
        expected_names.sort();

        assert_eq!(
            tool_names, expected_names,
            "Exposed planner tools do not match planner_tools::ALL_TOOLS"
        );
    }

    #[tokio::test]
    async fn test_worker_tools_consistency() {
        let config = crate::config::ZbobrConfig {
            domain_repo: "test/repo".to_string(),
            fork_owner: "test-owner".to_string(),
            default_model: "test-model".to_string(),
            workspace: std::path::PathBuf::from("/tmp"),
            github_token: "test-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: crate::config::CliTool::Stub,
        };
        let zbobr = Zbobr::new(config).unwrap();
        let worker = WorkerMcp::new(zbobr, 123);

        let tools = worker.tool_router.list_all();
        let mut tool_names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        tool_names.sort();

        let mut expected_names = worker_tools::ALL_TOOLS.to_vec();
        expected_names.sort();

        assert_eq!(
            tool_names, expected_names,
            "Exposed worker tools do not match worker_tools::ALL_TOOLS"
        );
    }
}

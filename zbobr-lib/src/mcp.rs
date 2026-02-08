use std::sync::Arc;

use crate::task::{Model, Role, Stage, Tool};
use crate::Zbobr;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};

// -- Parameter types --

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct DescriptionParam {
    #[schemars(description = "The task description/plan text")]
    pub description: String,
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
pub struct TaskIdParam {
    #[schemars(description = "The task ID")]
    pub id: u64,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct CreateTaskParam {
    #[schemars(description = "Task title")]
    pub title: String,
    #[schemars(description = "Task description")]
    pub description: String,
    #[schemars(description = "Task tool (optional)")]
    pub tool: Option<Tool>,
    #[schemars(description = "Task model (optional)")]
    pub model: Option<Model>,
    #[schemars(description = "Parent task ID (optional)")]
    pub parent_task_id: Option<u64>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct UpdateTaskParam {
    pub id: u64,
    pub description: String,
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
pub struct StageParam {
    #[schemars(description = "Stage name (e.g. PENDING, PLANNING_READY, etc.)")]
    pub stage: String,
    #[schemars(description = "Optional tool filter")]
    pub tool: Option<Tool>,
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
    GET_DESCRIPTION = "get_description",
    SET_DESCRIPTION = "set_description",
    GET_DISCUSSION = "get_discussion",
    POST_MESSAGE = "post_message",
    REQUEST_REPO = "request_repo",
}

mcp_tools! {
    worker_tools,
    GET_DESCRIPTION = "get_description",
    GET_DISCUSSION = "get_discussion",
    POST_MESSAGE = "post_message",
    REQUEST_REPO = "request_repo",
    SUBMIT_WORK = "submit_work",
    MARK_DONE = "mark_done",
}

mcp_tools! {
    admin_tools,
    LIST_TASKS = "list_tasks",
    CREATE_TASK = "create_task",
    GET_TASK = "get_task",
    UPDATE_TASK_DESCRIPTION = "update_task_description",
    SET_TASK_STAGE = "set_task_stage",
    ADD_TASK_LABEL = "add_task_label",
    REMOVE_TASK_LABEL = "remove_task_label",
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

    #[tool(description = "Get the current description/plan for this task")]
    async fn get_description(&self) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.get_description().await {
            Ok(desc) => desc,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Update the description/plan for this task")]
    async fn set_description(&self, Parameters(params): Parameters<DescriptionParam>) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.set_description(&params.description).await {
            Ok(()) => "Description updated successfully".to_string(),
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

    #[tool(description = "Get the current description/plan for this task")]
    async fn get_description(&self) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.get_description().await {
            Ok(desc) => desc,
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

    #[tool(description = "List tasks in a specific stage")]
    async fn list_tasks(&self, Parameters(params): Parameters<StageParam>) -> String {
        match self
            .zbobr
            .list_tasks_by_stage(&params.stage, params.tool)
            .await
        {
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

    #[tool(description = "Create a new task")]
    async fn create_task(&self, Parameters(params): Parameters<CreateTaskParam>) -> String {
        match self
            .zbobr
            .create_task(
                &params.title,
                &params.description,
                Stage::Planning,
                params.tool,
                params.model,
                params.parent_task_id,
                None,
                None,
            )
            .await
        {
            Ok(id) => format!("Created task #{}", id),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get details of a task")]
    async fn get_task(&self, Parameters(params): Parameters<TaskIdParam>) -> String {
        match self.zbobr.get_task(params.id).await {
            Ok(task) => format!(
                "ID: {}\nTitle: {}\nStage: {:?}\nTool: {:?}\nModel: {:?}\nDone: {}\n\n{}",
                task.id, task.title, task.stage, task.tool, task.model, task.done, task.description
            ),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Update the description of a task")]
    async fn update_task_description(
        &self,
        Parameters(params): Parameters<UpdateTaskParam>,
    ) -> String {
        match self
            .zbobr
            .update_task_description(params.id, &params.description)
            .await
        {
            Ok(()) => "Task description updated".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Change the stage of a task")]
    async fn set_task_stage(&self, Parameters(params): Parameters<SetStageParam>) -> String {
        match self
            .zbobr
            .set_task_stage_by_name(params.id, params.stage.milestone_name())
            .await
        {
            Ok(()) => format!("Stage updated to {}", params.stage),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Add a label to a task")]
    async fn add_task_label(&self, Parameters(params): Parameters<LabelParam>) -> String {
        match self.zbobr.add_task_label(params.id, &params.label).await {
            Ok(()) => format!("Label '{}' added", params.label),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Remove a label from a task")]
    async fn remove_task_label(&self, Parameters(params): Parameters<LabelParam>) -> String {
        match self.zbobr.remove_task_label(params.id, &params.label).await {
            Ok(()) => format!("Label '{}' removed", params.label),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get all logs/comments for a task")]
    async fn get_discussion(&self, Parameters(params): Parameters<TaskIdParam>) -> String {
        match self.zbobr.get_task_comments(params.id).await {
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
                "Admin tools: full control over tasks, statuses, and logs.".to_string(),
            ),
            ..Default::default()
        }
    }
}

async fn serve_mcp(
    port: u16,
    path: &str,
    router: axum::Router,
) -> Result<(), crate::ZbobrError> {
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

/// Run the MCP HTTP server scoped to a role (planner or worker) and task.
pub async fn run_role_mcp_server(
    zbobr: Zbobr,
    port: u16,
    role: Role,
    task_id: u64,
) -> Result<(), crate::ZbobrError> {
    let path = format!("/{}/{}", role, task_id);

    let router = match role {
        Role::Planner => {
            let svc = StreamableHttpService::new(
                move || Ok(PlannerMcp::new(zbobr.clone(), task_id)),
                Arc::new(LocalSessionManager::default()),
                Default::default(),
            );
            axum::Router::new().nest_service(&path, svc)
        }
        Role::Worker => {
            let svc = StreamableHttpService::new(
                move || Ok(WorkerMcp::new(zbobr.clone(), task_id)),
                Arc::new(LocalSessionManager::default()),
                Default::default(),
            );
            axum::Router::new().nest_service(&path, svc)
        }
    };

    serve_mcp(port, &path, router).await
}

/// Run the admin MCP HTTP server.
pub async fn run_admin_mcp_server(
    zbobr: Zbobr,
    port: u16,
) -> Result<(), crate::ZbobrError> {
    let path = "/admin";

    let svc = StreamableHttpService::new(
        move || Ok(AdminMcp::new(zbobr.clone())),
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );
    let router = axum::Router::new().nest_service(path, svc);

    serve_mcp(port, path, router).await
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
            default_model: Model::Gpt4o,
            workspace: std::path::PathBuf::from("/tmp"),
            github_token: "test-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: Tool::Stub,
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
            default_model: Model::Gpt4o,
            workspace: std::path::PathBuf::from("/tmp"),
            github_token: "test-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: Tool::Stub,
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
            default_model: Model::Gpt4o,
            workspace: std::path::PathBuf::from("/tmp"),
            github_token: "test-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: Tool::Stub,
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

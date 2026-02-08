use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use zbobr_lib::Zbobr;

// -- Parameter types --

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PlanParam {
    #[schemars(description = "The plan text")]
    pub plan: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct MessageParam {
    #[schemars(description = "The message to post")]
    pub message: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RepoParam {
    #[schemars(description = "Target repository in owner/name format")]
    pub repo: String,
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
    fn new(zbobr: Zbobr, task_id: u64) -> Self {
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

    #[tool(description = "Clone a repository for investigation (read-only). Returns the local path.")]
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
    fn new(zbobr: Zbobr, task_id: u64) -> Self {
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

    #[tool(description = "Fork and clone a repository for implementation. Returns the local path with feature branch ready.")]
    async fn request_repo(&self, Parameters(params): Parameters<RepoParam>) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.request_repo(&params.repo).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Push changes and create a PR from the feature branch. Takes the target repo (owner/name).")]
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

/// Run the MCP HTTP server with routing by URL path.
pub async fn run_mcp_server(zbobr: Zbobr, port: u16) -> anyhow::Result<()> {
    let zbobr = Arc::new(zbobr);

    let planner_zbobr = zbobr.clone();
    let planner_service = StreamableHttpService::new(
        move || Ok(PlannerMcp::new((*planner_zbobr).clone(), 0)),
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    let worker_zbobr = zbobr.clone();
    let worker_service = StreamableHttpService::new(
        move || Ok(WorkerMcp::new((*worker_zbobr).clone(), 0)),
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    let router = axum::Router::new()
        .nest_service("/planner", planner_service)
        .nest_service("/worker", worker_service);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    tracing::info!("MCP server listening on http://127.0.0.1:{port}");

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;

    Ok(())
}

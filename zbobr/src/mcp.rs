use std::sync::Arc;

use axum::extract::Path;
use rmcp::model::{ServerCapabilities, ServerInfo, Implementation};
use rmcp::handler::server::wrapper::tool::ToolCallContext;
use rmcp::{ServerHandler, tool};
use rmcp::model::CallToolResult;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use zbobr_lib::Zbobr;

/// Planner MCP service -- tools available to planner agents.
#[derive(Clone)]
pub struct PlannerMcp {
    zbobr: Zbobr,
    task_id: u64,
}

/// Worker MCP service -- tools available to worker agents.
#[derive(Clone)]
pub struct WorkerMcp {
    zbobr: Zbobr,
    task_id: u64,
}

// -- Planner tools --

#[tool(tool_box)]
impl PlannerMcp {
    #[tool(description = "Get the current plan text for this task")]
    async fn get_plan(&self) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.get_plan().await {
            Ok(plan) => plan,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Update the plan text for this task")]
    async fn set_plan(&self, #[tool(param)] plan: String) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.set_plan(&plan).await {
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
    async fn post_message(&self, #[tool(param)] message: String) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.post_message(&message).await {
            Ok(()) => "Message posted".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Clone a repository for investigation (read-only). Returns the local path.")]
    async fn request_repo(&self, #[tool(param)] repo: String) -> String {
        let session = self.zbobr.planner_session(self.task_id);
        match session.request_repo(&repo).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for PlannerMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Planner tools: investigate task and create implementation plan.".to_string(),
            ),
        }
    }
}

// -- Worker tools --

#[tool(tool_box)]
impl WorkerMcp {
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
    async fn post_message(&self, #[tool(param)] message: String) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.post_message(&message).await {
            Ok(()) => "Message posted".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Fork and clone a repository for implementation. Returns the local path with feature branch ready.")]
    async fn request_repo(&self, #[tool(param)] repo: String) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.request_repo(&repo).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Push changes and create a PR from the feature branch. Takes the target repo (owner/name).")]
    async fn submit_work(&self, #[tool(param)] target_repo: String) -> String {
        let session = self.zbobr.worker_session(self.task_id);
        match session.submit_work(&target_repo).await {
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

#[tool(tool_box)]
impl ServerHandler for WorkerMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Worker tools: implement task according to plan, submit work, mark done.".to_string(),
            ),
        }
    }
}

/// Run the MCP HTTP server with routing by URL path.
pub async fn run_mcp_server(zbobr: Zbobr, port: u16) -> anyhow::Result<()> {
    let zbobr = Arc::new(zbobr);

    // Create services for planner and worker, each parameterized by task_id from URL path

    let planner_zbobr = zbobr.clone();
    let planner_service = StreamableHttpService::new(
        move || {
            // Default task_id=0; will be overridden by the URL-path routing
            Ok(PlannerMcp {
                zbobr: (*planner_zbobr).clone(),
                task_id: 0,
            })
        },
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    let worker_zbobr = zbobr.clone();
    let worker_service = StreamableHttpService::new(
        move || {
            Ok(WorkerMcp {
                zbobr: (*worker_zbobr).clone(),
                task_id: 0,
            })
        },
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

use std::sync::Arc;

use crate::task::{Model, Role, Stage, Tool};
use crate::Zbobr;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde_json::Value;

/// Get the current hostname, or "unknown" if it cannot be determined.
fn get_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

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
pub struct PathParam {
    #[schemars(description = "Local filesystem path to repository")]
    pub path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct BranchParam {
    #[schemars(description = "Target repository in owner/name format")]
    pub repo: String,
    #[schemars(description = "Branch name to checkout")]
    pub branch: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct PrParam {
    #[schemars(description = "Pull request reference (URL like 'https://github.com/owner/repo/pull/123' or 'owner/repo#123')")]
    pub pr: String,
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
    #[schemars(description = "Stage name (e.g. PENDING, GO_PLANNING, etc.)")]
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
    GET_DISCUSSION = "get_discussion",
    POST_MESSAGE = "post_message",
    PULL_BRANCH = "pull_branch",
    PULL_BRANCH_BY_PR = "pull_branch_by_pr",
    PULL_WORK_BRANCH = "pull_work_branch",
}

mcp_tools! {
    worker_tools,
    GET_DESCRIPTION = "get_description",
    GET_DISCUSSION = "get_discussion",
    POST_MESSAGE = "post_message",
    GET_WORK_BRANCH_NAME = "get_work_branch_name",
    PULL_BRANCH = "pull_branch",
    PULL_BRANCH_BY_PR = "pull_branch_by_pr",
    PULL_WORK_BRANCH = "pull_work_branch",
    PUSH_WORK_BRANCH = "push_work_branch",
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

/// Generate hardcoded planner instructions using tool name constants.
pub fn planner_instructions() -> String {
    format!(
        r#"# Planner Agent

Investigate a task and create an implementation plan.

## Access Model

You can access the internet. Your restrictions:
- Do NOT write to GitHub — no git push, no `gh` write operations, no PRs
- Do NOT run git clone/pull/fetch — use MCP pull tools instead
- Use MCP tools to communicate results (plans, questions)
- Use `gh` CLI for read-only GitHub access when MCP tools are insufficient

Work autonomously. Do not ask the user for anything.

## Workflow

1. Call `{get_description}` to read the task
2. Call `{get_discussion}` for context and prior comments
3. Pull the relevant repository using one of:
   - `{pull_branch}` — pull any branch of any repository you need to investigate
   - `{pull_branch_by_pr}` — shortcut: if the task mentions a PR, pull it directly without reading the PR to find its branch
   - `{pull_work_branch}` — pull the task's work branch (to continue or review prior work)
4. Explore the codebase, understand the problem
5. Design a solution — focus on what and why, not detailed how
6. **REQUIRED**: Call `{post_message}` with your implementation plan in markdown

The plan is posted as a task comment. The worker agent will later retrieve it from the discussion.

## Plan Format

Post as markdown with sections: Overview, Changes Required (by repo/file), Testing Strategy, Risks."#,
        get_description = planner_tools::GET_DESCRIPTION,
        get_discussion = planner_tools::GET_DISCUSSION,
        post_message = planner_tools::POST_MESSAGE,
        pull_branch = planner_tools::PULL_BRANCH,
        pull_branch_by_pr = planner_tools::PULL_BRANCH_BY_PR,
        pull_work_branch = planner_tools::PULL_WORK_BRANCH,
    )
}

/// Generate hardcoded worker instructions using tool name constants.
pub fn worker_instructions() -> String {
    format!(
        r#"# Worker Agent

Implement an approved plan by writing code and submitting it.

## Access Model

You can access the internet and run local commands. Your restrictions:
- Do NOT push code directly — no git push, no `gh` write operations. Use `{push_work_branch}` instead
- Do NOT run git clone/pull/fetch — use MCP pull tools instead
- Use MCP tools to submit work and communicate results
- Use `gh` CLI for read-only GitHub access when MCP tools are insufficient

Work autonomously. Do not ask the user for anything.

## Workflow

1. Call `{get_description}` to read the task
2. Call `{get_discussion}` to retrieve the approved implementation plan (posted by the planner)
3. Set up the repository using one of:
   - `{pull_branch}` — pull any branch of any repository you need (forks automatically for write access)
   - `{pull_branch_by_pr}` — shortcut: if the task mentions a PR, pull it directly without reading the PR to find its branch
   - `{pull_work_branch}` — pull or create the task's work branch (for starting or continuing work)
4. `cd` into the returned path and implement the plan
5. Commit changes locally with clear messages
6. Call `{push_work_branch}` with the local path — this pushes to the fork and creates a PR automatically
7. If task is complete:
   - Call `{post_message}` to summarize what was done
   - Call `{mark_done}` to complete the task
8. If there are issues requiring user intervention:
   - Call `{post_message}` to describe the problem or question
   - Do NOT call `{mark_done}` — leave the task open for the user"#,
        get_description = worker_tools::GET_DESCRIPTION,
        get_discussion = worker_tools::GET_DISCUSSION,
        post_message = worker_tools::POST_MESSAGE,
        pull_branch = worker_tools::PULL_BRANCH,
        pull_branch_by_pr = worker_tools::PULL_BRANCH_BY_PR,
        pull_work_branch = worker_tools::PULL_WORK_BRANCH,
        push_work_branch = worker_tools::PUSH_WORK_BRANCH,
        mark_done = worker_tools::MARK_DONE,
    )
}

/// Generate concise API documentation from a tool router
fn generate_api_docs_from_router<T: Send + Sync + 'static>(router: &ToolRouter<T>, role_name: &str) -> String {
    let tools = router.list_all();

    let mut doc = format!("## {} MCP API\n\n", role_name);
    doc.push_str("Available tools (all pre-scoped to your task):\n\n");

    for tool in tools {
        doc.push_str(&format!("### `{}`\n\n", tool.name));
        doc.push_str(&format!("{}\n\n", tool.description.as_deref().unwrap_or("No description")));

        // Parameters
        let schema = &tool.input_schema;
        let properties_obj = schema.get("properties")
            .and_then(|v: &Value| v.as_object());

        if let Some(properties) = properties_obj {
                if !properties.is_empty() {
                    doc.push_str("**Parameters:**\n");
                    for (name, prop_val) in properties {
                        let required_arr = schema
                            .get("required")
                            .and_then(|v: &Value| v.as_array());
                        let required = required_arr
                            .map(|arr| arr.iter().any(|v: &Value| v.as_str() == Some(name.as_str())))
                            .unwrap_or(false);
                        let desc = match prop_val.get("description") {
                            Some(v) => v.as_str().unwrap_or(""),
                            None => "",
                        };
                        let type_str = match prop_val.get("type") {
                            Some(v) => v.as_str().unwrap_or("any"),
                            None => "any",
                        };
                        doc.push_str(&format!(
                            "- `{}` ({}{}) - {}\n",
                            name,
                            type_str,
                            if required { ", required" } else { "" },
                            desc
                        ));
                    }
                    doc.push('\n');
                } else {
                    doc.push_str("**Parameters:** None\n\n");
                }
            } else {
                doc.push_str("**Parameters:** None\n\n");
            }

        doc.push_str("---\n\n");
    }

    doc
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

    #[tool(description = "Get the current description/plan for this task (read-only)")]
    async fn get_description(&self) -> String {
        let session = self.zbobr.task_session(self.task_id);
        match session.get_description().await {
            Ok(desc) => desc,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get all discussion messages on this task")]
    async fn get_discussion(&self) -> String {
        let session = self.zbobr.task_session(self.task_id);
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
        let session = self.zbobr.task_session(self.task_id);
        let hostname = get_hostname();
        match session.post_message(&params.message, Role::Planner.as_str(), &hostname).await {
            Ok(()) => "Message posted".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Pull a repository and checkout a specific branch for investigation (read-only). Returns the local path."
    )]
    async fn pull_branch(&self, Parameters(params): Parameters<BranchParam>) -> String {
        let session = self.zbobr.task_session(self.task_id);
        match session.request_branch_readonly(&params.repo, &params.branch).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Pull a repository and checkout the branch from a PR (read-only). Takes PR URL or 'owner/repo#123' format. Returns the local path."
    )]
    async fn pull_branch_by_pr(&self, Parameters(params): Parameters<PrParam>) -> String {
        let session = self.zbobr.task_session(self.task_id);
        match session.request_branch_by_pr(&params.pr, true).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Pull the work branch for a repository. If the work branch (named by get_work_branch_name) exists in the fork, it will be pulled; otherwise, the main repository branch will be pulled and the work branch will be created locally. Returns the local path."
    )]
    async fn pull_work_branch(&self, Parameters(params): Parameters<RepoParam>) -> String {
        let session = self.zbobr.task_session(self.task_id);
        match session.request_work_branch(&params.repo).await {
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

impl PlannerMcp {
    /// Generate API documentation for planner tools
    pub fn generate_api_docs() -> String {
        let tools = Self::tool_router();
        generate_api_docs_from_router(&tools, "Planner")
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
        let session = self.zbobr.task_session(self.task_id);
        match session.get_description().await {
            Ok(desc) => desc,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get all discussion messages on this task")]
    async fn get_discussion(&self) -> String {
        let session = self.zbobr.task_session(self.task_id);
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
        let session = self.zbobr.task_session(self.task_id);
        let hostname = get_hostname();
        match session.post_message(&params.message, Role::Worker.as_str(), &hostname).await {
            Ok(()) => "Message posted".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get the work branch name that should be used for this task")]
    async fn get_work_branch_name(&self) -> String {
        let session = self.zbobr.task_session(self.task_id);
        session.get_work_branch_name()
    }

    #[tool(
        description = "Pull a repository (forking if needed) and checkout a specific branch for implementation. Returns the local path with feature branch ready."
    )]
    async fn pull_branch(&self, Parameters(params): Parameters<BranchParam>) -> String {
        let session = self.zbobr.task_session(self.task_id);
        match session.request_branch(&params.repo, &params.branch).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Pull a repository (forking if needed) and checkout the branch from a PR for implementation. Takes PR URL or 'owner/repo#123' format. Returns the local path."
    )]
    async fn pull_branch_by_pr(&self, Parameters(params): Parameters<PrParam>) -> String {
        let session = self.zbobr.task_session(self.task_id);
        match session.request_branch_by_pr(&params.pr, false).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Pull the work branch for a repository. If the work branch (named by get_work_branch_name) exists in the fork, it will be pulled; otherwise, the main repository branch will be pulled and the work branch will be created locally. Returns the local path."
    )]
    async fn pull_work_branch(&self, Parameters(params): Parameters<RepoParam>) -> String {
        let session = self.zbobr.task_session(self.task_id);
        match session.request_work_branch(&params.repo).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        description = "Push the current branch in the repository to the fork with the work branch name (from get_work_branch_name) and create a PR. Takes the local path to the repository (from pull_branch, pull_branch_by_pr, or pull_work_branch). Returns PR URL."
    )]
    async fn push_work_branch(&self, Parameters(params): Parameters<PathParam>) -> String {
        let session = self.zbobr.task_session(self.task_id);
        match session.submit_work(&params.path).await {
            Ok(pr_url) => format!("PR created: {pr_url}"),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Mark this task as done")]
    async fn mark_done(&self) -> String {
        let session = self.zbobr.task_session(self.task_id);
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

impl WorkerMcp {
    /// Generate API documentation for worker tools
    pub fn generate_api_docs() -> String {
        let tools = Self::tool_router();
        generate_api_docs_from_router(&tools, "Worker")
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

/// Find an available port starting from the given base port.
/// Tries ports incrementally until one is available.
async fn find_available_port(base_port: u16) -> Result<u16, crate::ZbobrError> {
    for port in base_port..=base_port + 100 {
        match tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await {
            Ok(_) => return Ok(port),
            Err(_) => continue,
        }
    }
    Err(crate::ZbobrError::Other(
        format!("Could not find available port in range {base_port}..{}", base_port + 100),
    ))
}

async fn serve_mcp(
    base_port: u16,
    path: &str,
    router: axum::Router,
) -> Result<u16, crate::ZbobrError> {
    // Find an available port starting from base_port
    let port = find_available_port(base_port).await?;
    
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    tracing::info!("MCP server listening on http://127.0.0.1:{port}{path}");

    // Spawn the actual server in a background task
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c().await.ok();
            })
            .await
        {
            tracing::error!("Axum server error: {e}");
        }
    });

    Ok(port)
}

/// Run the MCP HTTP server scoped to a role (planner or worker) and task.
/// Returns the actual port that was assigned (spawns server in background).
pub async fn run_role_mcp_server(
    zbobr: Zbobr,
    base_port: u16,
    role: Role,
    task_id: u64,
) -> Result<u16, crate::ZbobrError> {
    let path = format!("/{}/{}", role, task_id);

    let router = match role {
        Role::Planner => {
            tracing::info!("Creating PlannerMcp service for task {task_id} at path {path}");
            let svc = StreamableHttpService::new(
                move || {
                    tracing::debug!("Creating new PlannerMcp instance for task {task_id}");
                    Ok(PlannerMcp::new(zbobr.clone(), task_id))
                },
                Arc::new(LocalSessionManager::default()),
                Default::default(),
            );
            axum::Router::new().nest_service(&path, svc)
        }
        Role::Worker => {
            tracing::info!("Creating WorkerMcp service for task {task_id} at path {path}");
            let svc = StreamableHttpService::new(
                move || {
                    tracing::debug!("Creating new WorkerMcp instance for task {task_id}");
                    Ok(WorkerMcp::new(zbobr.clone(), task_id))
                },
                Arc::new(LocalSessionManager::default()),
                Default::default(),
            );
            axum::Router::new().nest_service(&path, svc)
        }
    };

    serve_mcp(base_port, &path, router).await
}

/// Run the admin MCP HTTP server.
/// Returns the actual port that was assigned.
pub async fn run_admin_mcp_server(
    zbobr: Zbobr,
    base_port: u16,
) -> Result<u16, crate::ZbobrError> {
    let path = "/admin";

    let svc = StreamableHttpService::new(
        move || Ok(AdminMcp::new(zbobr.clone())),
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );
    let router = axum::Router::new().nest_service(path, svc);

    serve_mcp(base_port, path, router).await
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
            owner_github_token: "owner-token".to_string(),
            agent_github_token: "agent-token".to_string(),
            copilot_github_token: "copilot-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: Tool::Stub,
            planner_prompts: vec![],
            worker_prompts: vec![],
            work_branch_prefix: "zbobr_fix".to_string(),
            prompts_path: None,
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
            owner_github_token: "owner-token".to_string(),
            agent_github_token: "agent-token".to_string(),
            copilot_github_token: "copilot-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: Tool::Stub,
            planner_prompts: vec![],
            worker_prompts: vec![],
            work_branch_prefix: "zbobr_fix".to_string(),
            prompts_path: None,
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
            owner_github_token: "owner-token".to_string(),
            agent_github_token: "agent-token".to_string(),
            copilot_github_token: "copilot-token".to_string(),
            backend: crate::config::BackendType::Stub,
            cli_tool: Tool::Stub,
            planner_prompts: vec![],
            worker_prompts: vec![],
            work_branch_prefix: "zbobr_fix".to_string(),
            prompts_path: None,
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

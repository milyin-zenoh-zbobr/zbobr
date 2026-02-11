use std::sync::Arc;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    },
    ServerHandler,
};
use serde_json::Value;

use crate::{
    task::{Model, ChecklistItem, Role, Stage, TaskSession, Tool},
    Zbobr,
};

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
    #[schemars(
        description = "Pull request reference (URL like 'https://github.com/owner/repo/pull/123' or 'owner/repo#123')"
    )]
    pub pr: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct ShortNameParam {
    #[schemars(description = "Short name for the branch (e.g. 'implementation', 'fix-typo')")]
    pub short_name: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct PushBranchAndCreatePrParam {
    #[schemars(description = "Local filesystem path to repository")]
    pub path: String,
    #[schemars(description = "Destination branch for the PR base (e.g. 'main')")]
    pub destination_branch: String,
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
pub struct StageParam {
    #[schemars(description = "Stage name (e.g. PENDING, GO_PLANNING, etc.)")]
    pub stage: String,
    #[schemars(description = "Optional tool filter")]
    pub tool: Option<Tool>,
}

// -- Checklist parameter types --

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct InsertChecklistItemParam {
    #[schemars(description = "Unique identifier for the new checklist item")]
    pub id: String,
    #[schemars(description = "Checklist item text")]
    pub text: String,
    #[schemars(description = "Optional ID of the item to insert after (if omitted, adds to end)")]
    pub after_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct UpdateChecklistItemParam {
    #[schemars(description = "ID of the checklist item to update")]
    pub id: String,
    #[schemars(description = "New text for the checklist item")]
    pub text: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct CheckChecklistItemParam {
    #[schemars(description = "ID of the checklist item to check/uncheck")]
    pub id: String,
    #[schemars(description = "New checkbox state (true = checked, false = unchecked)")]
    pub checked: bool,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct DeleteChecklistItemParam {
    #[schemars(description = "ID of the checklist item to delete")]
    pub id: String,
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
    GET_PLAN = "get_plan",
    POST_PLAN = "post_plan",
    POST_MESSAGE = "post_message",
    PULL_BRANCH = "pull_branch",
    PULL_BRANCH_BY_PR = "pull_branch_by_pr",
    GET_CHECKLIST = "get_checklist",
    INSERT_CHECKLIST_ITEM = "insert_checklist_item",
    UPDATE_CHECKLIST_ITEM = "update_checklist_item",
    CHECK_CHECKLIST_ITEM = "check_checklist_item",
    DELETE_CHECKLIST_ITEM = "delete_checklist_item",
}

mcp_tools! {
    worker_tools,
    GET_DESCRIPTION = "get_description",
    GET_DISCUSSION = "get_discussion",
    GET_PLAN = "get_plan",
    POST_MESSAGE = "post_message",
    POST_QUESTION = "post_question",
    CREATE_BRANCH_NAME = "create_branch_name",
    PULL_BRANCH = "pull_branch",
    PULL_BRANCH_BY_PR = "pull_branch_by_pr",
    PUSH_BRANCH = "push_branch",
    PUSH_BRANCH_AND_CREATE_PR = "push_branch_and_create_pr",
    GET_CHECKLIST = "get_checklist",
    CHECK_CHECKLIST_ITEM = "check_checklist_item",
}

mcp_tools! {
    admin_tools,
    LIST_TASKS = "list_tasks",
    CREATE_TASK = "create_task",
    GET_TASK = "get_task",
    UPDATE_TASK_DESCRIPTION = "update_task_description",
    SET_TASK_STAGE = "set_task_stage",
    GET_DISCUSSION = "get_discussion",
    DEBUG_STATE = "debug_state",
}

/// Generate hardcoded planner instructions using tool name constants.
pub fn planner_instructions() -> String {
    format!(
        r#"# Planner Agent

Investigate a task and create an implementation plan.

## Access Model

You can access the internet and run local commands. Your restrictions:
- Do NOT run git clone/pull/fetch — use `{pull_branch}` or `{pull_branch_by_pr}` instead
- Use MCP `{post_plan}` to post the implementation plan
- Use MCP `{post_message}` to communicate results and questions
- For reading GitHub data: use `git` and `gh` CLI only when no MCP tool provides the needed information
- NEVER use git/gh for writing, pushing, or sending data to GitHub

Work autonomously. Do not ask the user for anything.

## Workflow

1. Call `{get_description}` to read the task
2. Call `{get_discussion}` for context and prior comments
3. Pull the relevant repository using one of:
   - `{pull_branch}` — pull any branch of any repository you need to investigate
   - `{pull_branch_by_pr}` — shortcut: if the task mentions a PR, pull it directly without reading the PR to find its branch
4. Explore the codebase, understand the problem
5. Design a solution — focus on what and why, not detailed how
6. **REQUIRED**: Call `{post_plan}` with your implementation plan in markdown

The plan is stored as a separate field in the task (between description and checklist). The worker agent will later retrieve it using `{get_plan}`.

## Plan Format

Post markdown with sections: Overview, Changes Required (by repo/file), Testing Strategy, Risks.
You can also create checklist items separately if needed using the checklist operations."#,
        get_description = planner_tools::GET_DESCRIPTION,
        get_discussion = planner_tools::GET_DISCUSSION,
        get_plan = planner_tools::GET_PLAN,
        post_plan = planner_tools::POST_PLAN,
        post_message = planner_tools::POST_MESSAGE,
        pull_branch = planner_tools::PULL_BRANCH,
        pull_branch_by_pr = planner_tools::PULL_BRANCH_BY_PR,
    )
}

/// Generate hardcoded worker instructions using tool name constants.
pub fn worker_instructions() -> String {
    format!(
        r#"# Worker Agent

Implement an approved plan by writing code and submitting it.

## Access Model

You can access the internet and run local commands. Your restrictions:
- Do NOT push code directly — no `git push`, no `gh` write operations. Use `{push_branch}` or `{push_branch_and_create_pr}` instead. Access rights are configured to prevent gh-based pushes anyway.
- Do NOT run git clone/pull/fetch — use `{pull_branch}` or `{pull_branch_by_pr}` instead
- Use MCP `{push_branch_and_create_pr}` to submit your work
- Use MCP `{post_message}`, `{post_question}` to communicate results
- For reading GitHub data: use `git` and `gh` CLI only when no MCP tool provides the needed information
- NEVER use git/gh for writing, pushing, or sending data to GitHub

Work autonomously. Do not ask the user for anything.

## Workflow

1. Call `{get_description}` to read the task
2. Call `{get_plan}` to retrieve the approved implementation plan (posted by the planner)
3. Call `{get_discussion}` if you need additional context from comments
4. Set up the repository using one of:
   - `{pull_branch}` — pull any branch of any repository you need (forks automatically for write access)
   - `{pull_branch_by_pr}` — shortcut: if the task mentions a PR, pull it directly without reading the PR to find its branch
5. `cd` into the returned path and implement the plan
6. **REQUIRED**: Create a branch using `git checkout -b <name>` where `<name>` **must** come from `{create_branch_name}` (e.g. with short_name="implementation"). Do NOT use arbitrary branch names.
7. Commit changes locally with clear messages
8. Call `{push_branch_and_create_pr}` with local path and destination branch — this pushes to the fork and creates a PR within the fork
   - Or call `{push_branch}` if you only need to push without creating a PR
9. When you complete implementation steps:
   - Update all checklist items to checked state as you complete them
   - Call `{post_message}` to summarize what was accomplished
10. If there are issues requiring user intervention:
    - Call `{post_message}` to describe the problem
    - Call `{post_question}` to post a question and request human input"#,
        get_description = worker_tools::GET_DESCRIPTION,
        get_discussion = worker_tools::GET_DISCUSSION,
        get_plan = worker_tools::GET_PLAN,
        post_message = worker_tools::POST_MESSAGE,
        create_branch_name = worker_tools::CREATE_BRANCH_NAME,
        pull_branch = worker_tools::PULL_BRANCH,
        pull_branch_by_pr = worker_tools::PULL_BRANCH_BY_PR,
        push_branch = worker_tools::PUSH_BRANCH,
        push_branch_and_create_pr = worker_tools::PUSH_BRANCH_AND_CREATE_PR,
        post_question = worker_tools::POST_QUESTION,
    )
}

/// Generate concise API documentation from a tool router
fn generate_api_docs_from_router<T: Send + Sync + 'static>(
    router: &ToolRouter<T>,
    role_name: &str,
) -> String {
    let tools = router.list_all();

    let mut doc = format!("## {} MCP API\n\n", role_name);
    doc.push_str("Available tools (all pre-scoped to your task):\n\n");

    for tool in tools {
        doc.push_str(&format!("### `{}`\n\n", tool.name));
        doc.push_str(&format!(
            "{}\n\n",
            tool.description.as_deref().unwrap_or("No description")
        ));

        // Parameters
        let schema = &tool.input_schema;
        let properties_obj = schema.get("properties").and_then(|v: &Value| v.as_object());

        if let Some(properties) = properties_obj {
            if !properties.is_empty() {
                doc.push_str("**Parameters:**\n");
                for (name, prop_val) in properties {
                    let required_arr = schema.get("required").and_then(|v: &Value| v.as_array());
                    let required = required_arr
                        .map(|arr| {
                            arr.iter()
                                .any(|v: &Value| v.as_str() == Some(name.as_str()))
                        })
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

// -- MCP Trait Hierarchy --

/// Common trait for MCP services (Planner, Worker) - shared implementations
pub trait CommonMcpImpl: Send + Sync {
    fn session(&self) -> &TaskSession;
    fn role(&self) -> Role;
    
    fn role_name(&self) -> &'static str {
        self.role().as_str()
    }
    
    async fn get_description_impl(&self) -> String {
        tracing::info!("[{}#{}] get_description", self.role_name(), self.session().task_id());
        match self.session().get_description().await {
            Ok(desc) => desc,
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn get_discussion_impl(&self) -> String {
        tracing::info!("[{}#{}] get_discussion", self.role_name(), self.session().task_id());
        match self.session().get_discussion().await {
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

    async fn post_message_impl(&self, message: &str) -> String {
        tracing::info!("[{}#{}] post_message", self.role_name(), self.session().task_id());
        let hostname = get_hostname();
        match self
            .session()
            .post_message(message, self.role().as_str(), &hostname)
            .await
        {
            Ok(()) => "Message posted".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn get_plan_impl(&self) -> String {
        tracing::info!("[{}#{}] get_plan", self.role_name(), self.session().task_id());
        match self.session().get_plan().await {
            Ok(plan) => plan,
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn get_checklist_impl(&self) -> String {
        tracing::info!("[{}#{}] get_checklist", self.role_name(), self.session().task_id());
        match self.session().get_checklist().await {
            Ok(items) => {
                match serde_json::to_string_pretty(&items) {
                    Ok(json) => json,
                    Err(e) => format!("Error serializing checklist: {e}"),
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn check_checklist_item_impl(&self, id: &str, checked: bool) -> String {
        tracing::info!("[{}#{}] check_checklist_item id={} checked={}", self.role_name(), self.session().task_id(), id, checked);
        match (self.session().get_description().await, self.session().get_checklist().await) {
            (Ok(desc), Ok(mut items)) => {
                if let Some(item) = items.iter_mut().find(|item| item.id == id) {
                    item.checked = checked;
                    
                    match self.session().update_checklist(&desc, &items).await {
                        Ok(()) => {
                            // Determine signal based on checklist state
                            let has_unchecked = items.iter().any(|i| !i.checked);
                            let signal = if has_unchecked {
                                crate::Signal::GoWork
                            } else {
                                crate::Signal::Done
                            };
                            
                            if let Err(e) = self.session().set_signal(signal).await {
                                return format!("Checklist item '{}' checked state updated to {} but error setting signal: {}", id, checked, e);
                            }
                            
                            format!("Checklist item '{}' checked state updated to {}", id, checked)
                        }
                        Err(e) => format!("Error updating task: {e}"),
                    }
                } else {
                    format!("Error: Checklist item with id '{}' not found", id)
                }
            }
            (Err(e), _) | (_, Err(e)) => format!("Error: {e}"),
        }
    }
}

/// Planner-specific MCP implementations
pub trait PlannerMcpImpl: CommonMcpImpl {
    async fn post_plan_impl(&self, plan: &str) -> String {
        tracing::info!("[planner#{}] post_plan", self.session().task_id());
        match (self.session().get_description().await, self.session().get_checklist().await) {
            (Ok(desc), Ok(items)) => {
                match self.session().update_plan(&desc, plan, &items).await {
                    Ok(()) => {
                        // Set signal to go_work after posting plan
                        if let Err(e) = self.session().set_signal(crate::Signal::GoWork).await {
                            return format!("Plan posted but error setting signal: {e}");
                        }
                        "Plan posted/updated".to_string()
                    }
                    Err(e) => format!("Error updating task: {e}"),
                }
            }
            (Err(e), _) | (_, Err(e)) => format!("Error: {e}"),
        }
    }

    async fn pull_branch_impl(&self, repo: &str, branch: &str) -> String {
        tracing::info!("[planner#{}] pull_branch repo={} branch={}", self.session().task_id(), repo, branch);
        match self.session().request_branch_readonly(repo, branch).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn pull_branch_by_pr_impl(&self, pr: &str) -> String {
        tracing::info!("[planner#{}] pull_branch_by_pr pr={}", self.session().task_id(), pr);
        match self.session().request_branch_by_pr(pr, true).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn insert_checklist_item_impl(&self, id: &str, after_id: Option<String>, text: &str) -> String {
        tracing::info!("[planner#{}] insert_checklist_item id={} after_id={:?}", self.session().task_id(), id, after_id);
        match (self.session().get_description().await, self.session().get_checklist().await) {
            (Ok(desc), Ok(mut items)) => {
                if items.iter().any(|item| item.id == id) {
                    return format!("Error: Checklist item with id '{}' already exists", id);
                }
                
                let new_item = ChecklistItem {
                    id: id.to_string(),
                    checked: false,
                    text: text.to_string(),
                };
                
                if let Some(after_id) = after_id {
                    if let Some(pos) = items.iter().position(|item| item.id == after_id) {
                        items.insert(pos + 1, new_item);
                    } else {
                        return format!("Error: Checklist item with id '{}' not found", after_id);
                    }
                } else {
                    items.push(new_item);
                }
                
                match self.session().update_checklist(&desc, &items).await {
                    Ok(()) => format!("Checklist item '{}' inserted", id),
                    Err(e) => format!("Error updating task: {e}"),
                }
            }
            (Err(e), _) | (_, Err(e)) => format!("Error: {e}"),
        }
    }

    async fn update_checklist_item_impl(&self, id: &str, text: &str) -> String {
        tracing::info!("[planner#{}] update_checklist_item id={}", self.session().task_id(), id);
        match (self.session().get_description().await, self.session().get_checklist().await) {
            (Ok(desc), Ok(mut items)) => {
                if let Some(item) = items.iter_mut().find(|item| item.id == id) {
                    item.text = text.to_string();
                    
                    match self.session().update_checklist(&desc, &items).await {
                        Ok(()) => format!("Checklist item '{}' updated", id),
                        Err(e) => format!("Error updating task: {e}"),
                    }
                } else {
                    format!("Error: Checklist item with id '{}' not found", id)
                }
            }
            (Err(e), _) | (_, Err(e)) => format!("Error: {e}"),
        }
    }

    async fn delete_checklist_item_impl(&self, id: &str) -> String {
        tracing::info!("[planner#{}] delete_checklist_item id={}", self.session().task_id(), id);
        match (self.session().get_description().await, self.session().get_checklist().await) {
            (Ok(desc), Ok(mut items)) => {
                let original_len = items.len();
                items.retain(|item| item.id != id);
                
                if items.len() == original_len {
                    return format!("Error: Checklist item with id '{}' not found", id);
                }
                
                match self.session().update_checklist(&desc, &items).await {
                    Ok(()) => format!("Checklist item '{}' deleted", id),
                    Err(e) => format!("Error updating task: {e}"),
                }
            }
            (Err(e), _) | (_, Err(e)) => format!("Error: {e}"),
        }
    }
}

/// Worker-specific MCP implementations
pub trait WorkerMcpImpl: CommonMcpImpl {
    async fn post_question_impl(&self, message: &str) -> String {
        tracing::info!("[worker#{}] post_question", self.session().task_id());
        let hostname = get_hostname();
        
        if let Err(e) = self.session().post_message(message, self.role().as_str(), &hostname).await {
            return format!("Error posting message: {e}");
        }
        
        // Set signal to go_ask after posting question
        if let Err(e) = self.session().set_signal(crate::Signal::GoAsk).await {
            return format!("Question posted but error setting signal: {e}");
        }
        "Question posted and signal set".to_string()
    }

    async fn create_branch_name_impl(&self, short_name: &str) -> String {
        tracing::info!("[worker#{}] create_branch_name short_name={}", self.session().task_id(), short_name);
        self.session().create_branch_name(short_name)
    }

    async fn pull_branch_impl(&self, repo: &str, branch: &str) -> String {
        tracing::info!("[worker#{}] pull_branch repo={} branch={}", self.session().task_id(), repo, branch);
        match self.session().request_branch(repo, branch).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn pull_branch_by_pr_impl(&self, pr: &str) -> String {
        tracing::info!("[worker#{}] pull_branch_by_pr pr={}", self.session().task_id(), pr);
        match self.session().request_branch_by_pr(pr, false).await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn push_branch_impl(&self, path: &str) -> String {
        tracing::info!("[worker#{}] push_branch path={}", self.session().task_id(), path);
        match self.session().push_branch(path).await {
            Ok(()) => "Branch pushed to fork".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn push_branch_and_create_pr_impl(&self, path: &str, destination_branch: &str) -> String {
        tracing::info!("[worker#{}] push_branch_and_create_pr path={} destination_branch={}", self.session().task_id(), path, destination_branch);
        match self.session().push_branch_and_create_pr(path, destination_branch).await {
            Ok(pr_url) => format!("PR created: {pr_url}"),
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn mark_done_impl(&self) -> String {
        tracing::info!("[worker#{}] mark_done", self.session().task_id());
        match self.session().mark_done().await {
            Ok(()) => "Task marked as done".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }
}

// -- Planner MCP service --

#[derive(Clone)]
pub struct PlannerMcp {
    session: TaskSession,
    tool_router: ToolRouter<Self>,
}

impl CommonMcpImpl for PlannerMcp {
    fn session(&self) -> &TaskSession {
        &self.session
    }
    
    fn role(&self) -> Role {
        Role::Planner
    }
}

impl PlannerMcpImpl for PlannerMcp {}

#[tool_router]
impl PlannerMcp {
    pub fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self {
            session: zbobr.task_session(task_id),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get the current description for this task (read-only)")]
    async fn get_description(&self) -> String {
        self.get_description_impl().await
    }

    #[tool(description = "Get all discussion messages on this task")]
    async fn get_discussion(&self) -> String {
        self.get_discussion_impl().await
    }

    #[tool(description = "Post a message to the task discussion")]
    async fn post_message(&self, Parameters(params): Parameters<MessageParam>) -> String {
        self.post_message_impl(&params.message).await
    }

    #[tool(description = "Get the current implementation plan for this task")]
    async fn get_plan(&self) -> String {
        self.get_plan_impl().await
    }

    #[tool(description = "Post or replace the implementation plan for this task")]
    async fn post_plan(&self, Parameters(params): Parameters<DescriptionParam>) -> String {
        self.post_plan_impl(&params.description).await
    }

    #[tool(
        description = "Pull a repository and checkout a specific branch for investigation (read-only). Returns the local path."
    )]
    async fn pull_branch(&self, Parameters(params): Parameters<BranchParam>) -> String {
        self.pull_branch_impl(&params.repo, &params.branch).await
    }

    #[tool(
        description = "Pull a repository and checkout the branch from a PR (read-only). Takes PR URL or 'owner/repo#123' format. Returns the local path."
    )]
    async fn pull_branch_by_pr(&self, Parameters(params): Parameters<PrParam>) -> String {
        self.pull_branch_by_pr_impl(&params.pr).await
    }

    #[tool(description = "Get the task checklist as a list of checkbox items")]
    async fn get_checklist(&self) -> String {
        self.get_checklist_impl().await
    }

    #[tool(description = "Insert a new checklist item (always created in unchecked state)")]
    async fn insert_checklist_item(&self, Parameters(params): Parameters<InsertChecklistItemParam>) -> String {
        self.insert_checklist_item_impl(&params.id, params.after_id.clone(), &params.text).await
    }

    #[tool(description = "Update a checklist item's text")]
    async fn update_checklist_item(&self, Parameters(params): Parameters<UpdateChecklistItemParam>) -> String {
        self.update_checklist_item_impl(&params.id, &params.text).await
    }

    #[tool(description = "Check or uncheck a checklist item")]
    async fn check_checklist_item(&self, Parameters(params): Parameters<CheckChecklistItemParam>) -> String {
        self.check_checklist_item_impl(&params.id, params.checked).await
    }

    #[tool(description = "Delete a checklist item")]
    async fn delete_checklist_item(&self, Parameters(params): Parameters<DeleteChecklistItemParam>) -> String {
        self.delete_checklist_item_impl(&params.id).await
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
    session: TaskSession,
    tool_router: ToolRouter<Self>,
}

impl CommonMcpImpl for WorkerMcp {
    fn session(&self) -> &TaskSession {
        &self.session
    }
    
    fn role(&self) -> Role {
        Role::Worker
    }
}

impl WorkerMcpImpl for WorkerMcp {}

#[tool_router]
impl WorkerMcp {
    pub fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self {
            session: zbobr.task_session(task_id),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get the current description for this task")]
    async fn get_description(&self) -> String {
        self.get_description_impl().await
    }

    #[tool(description = "Get all discussion messages on this task")]
    async fn get_discussion(&self) -> String {
        self.get_discussion_impl().await
    }

    #[tool(description = "Post a message to the task discussion")]
    async fn post_message(&self, Parameters(params): Parameters<MessageParam>) -> String {
        self.post_message_impl(&params.message).await
    }

    #[tool(description = "Post a question to the task discussion and set the 'question' label")]
    async fn post_question(&self, Parameters(params): Parameters<MessageParam>) -> String {
        self.post_question_impl(&params.message).await
    }

    #[tool(description = "Get the current implementation plan for this task")]
    async fn get_plan(&self) -> String {
        self.get_plan_impl().await
    }

    #[tool(
        description = "Generate a branch name with the proper prefix for this task. Use the returned name with 'git checkout -b <name>' to create the branch locally."
    )]
    async fn create_branch_name(&self, Parameters(params): Parameters<ShortNameParam>) -> String {
        self.create_branch_name_impl(&params.short_name).await
    }

    #[tool(
        description = "Pull a repository (forking if needed) and checkout a specific branch for implementation. Returns the local path."
    )]
    async fn pull_branch(&self, Parameters(params): Parameters<BranchParam>) -> String {
        self.pull_branch_impl(&params.repo, &params.branch).await
    }

    #[tool(
        description = "Pull a repository (forking if needed) and checkout the branch from a PR for implementation. Takes PR URL or 'owner/repo#123' format. Returns the local path."
    )]
    async fn pull_branch_by_pr(&self, Parameters(params): Parameters<PrParam>) -> String {
        self.pull_branch_by_pr_impl(&params.pr).await
    }

    #[tool(
        description = "Push the current branch to the fork remote. REQUIREMENT: The branch name must have been created using create_branch_name() — branches with other names are rejected. Takes the local path to the repository."
    )]
    async fn push_branch(&self, Parameters(params): Parameters<PathParam>) -> String {
        self.push_branch_impl(&params.path).await
    }

    #[tool(
        description = "Push the current branch to the fork and create a PR within the fork. REQUIREMENT: The branch name must have been created using create_branch_name() — branches with other names are rejected. Takes the local path and destination branch for the PR base. Returns PR URL."
    )]
    async fn push_branch_and_create_pr(
        &self,
        Parameters(params): Parameters<PushBranchAndCreatePrParam>,
    ) -> String {
        self.push_branch_and_create_pr_impl(&params.path, &params.destination_branch).await
    }

    #[tool(description = "Get the task checklist as a list of checkbox items")]
    async fn get_checklist(&self) -> String {
        self.get_checklist_impl().await
    }

    #[tool(description = "Check or uncheck a checklist item")]
    async fn check_checklist_item(&self, Parameters(params): Parameters<CheckChecklistItemParam>) -> String {
        self.check_checklist_item_impl(&params.id, params.checked).await
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
        tracing::info!(
            "[admin] list_tasks stage={} tool={:?}",
            params.stage,
            params.tool
        );
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
        tracing::info!("[admin] create_task title={}", params.title);
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
        tracing::info!("[admin] get_task id={}", params.id);
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
        tracing::info!("[admin] update_task_description id={}", params.id);
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
        tracing::info!(
            "[admin] set_task_stage id={} stage={}",
            params.id,
            params.stage
        );
        match self
            .zbobr
            .set_task_stage_by_name(params.id, params.stage.milestone_name())
            .await
        {
            Ok(()) => format!("Stage updated to {}", params.stage),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get all logs/comments for a task")]
    async fn get_discussion(&self, Parameters(params): Parameters<TaskIdParam>) -> String {
        tracing::info!("[admin] get_discussion id={}", params.id);
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
        tracing::info!("[admin] debug_state");
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
    Err(crate::ZbobrError::Other(format!(
        "Could not find available port in range {base_port}..{}",
        base_port + 100
    )))
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
pub async fn run_admin_mcp_server(zbobr: Zbobr, base_port: u16) -> Result<u16, crate::ZbobrError> {
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

    #[test]
    fn test_checklist_parsing_and_serialization() {
        use crate::backend::{parse_description_with_checklist, serialize_description_with_checklist};
        
        // Test with no checklist
        let desc = "This is a task description";
        let (original, items) = parse_description_with_checklist(desc);
        assert_eq!(original, desc);
        assert!(items.is_empty());

        // Test with checklist
        let desc_with_checklist = "Task description\n---CHECKLIST---\n- [ ] item1: First item\n- [x] item2: Second item checked\n- [ ] item3: Third item\n";
        let (original, items) = parse_description_with_checklist(desc_with_checklist);
        assert_eq!(original, "Task description");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id, "item1");
        assert_eq!(items[0].text, "First item");
        assert!(!items[0].checked);
        assert_eq!(items[1].id, "item2");
        assert_eq!(items[1].text, "Second item checked");
        assert!(items[1].checked);
        assert_eq!(items[2].id, "item3");
        assert_eq!(items[2].text, "Third item");
        assert!(!items[2].checked);

        // Test serialization
        let serialized = serialize_description_with_checklist(&original, &items);
        assert!(serialized.contains("Task description"));
        assert!(serialized.contains("---CHECKLIST---"));
        assert!(serialized.contains("- [ ] item1: First item"));
        assert!(serialized.contains("- [x] item2: Second item checked"));
        assert!(serialized.contains("- [ ] item3: Third item"));

        // Test round-trip
        let (original2, items2) = parse_description_with_checklist(&serialized);
        assert_eq!(original, original2);
        assert_eq!(items.len(), items2.len());
        for (item1, item2) in items.iter().zip(items2.iter()) {
            assert_eq!(item1.id, item2.id);
            assert_eq!(item1.text, item2.text);
            assert_eq!(item1.checked, item2.checked);
        }
    }

    #[test]
    fn test_description_checklist_validation() {
        use crate::backend::{parse_description_with_checklist, serialize_description_with_checklist, strip_checklist_from_description};
        
        // Test stripping existing checklist from description
        let desc_with_old_checklist = "Task description\n---CHECKLIST---\n- [ ] old1: Old item\n";
        let stripped = strip_checklist_from_description(desc_with_old_checklist);
        assert_eq!(stripped, "Task description");
        
        // Test that serialize_description_with_checklist replaces old checklist with new one
        let new_checklist = vec![
            ChecklistItem { id: "new1".to_string(), checked: false, text: "New item".to_string() },
        ];
        let serialized = serialize_description_with_checklist(desc_with_old_checklist, &new_checklist);
        
        // Should contain the new checklist, not the old one
        assert!(serialized.contains("- [ ] new1: New item"));
        assert!(!serialized.contains("old1"));
        
        // Should parse correctly
        let (original, items) = parse_description_with_checklist(&serialized);
        assert_eq!(original, "Task description");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "new1");
        assert_eq!(items[0].text, "New item");
    }

    #[test]
    fn test_plan_parsing_and_serialization() {
        use crate::backend::{parse_description_with_plan_and_checklist, serialize_description_with_plan_and_checklist, extract_plan};
        
        // Test with no plan or checklist
        let desc = "This is a task description";
        let (original, plan, items) = parse_description_with_plan_and_checklist(desc);
        assert_eq!(original, desc);
        assert_eq!(plan, "");
        assert!(items.is_empty());

        // Test with plan and checklist
        let full_text = "Task description\n---PLAN---\nImplementation plan here\n---CHECKLIST---\n- [ ] item1: First item\n- [x] item2: Done item\n";
        let (original, plan, items) = parse_description_with_plan_and_checklist(full_text);
        assert_eq!(original, "Task description");
        assert_eq!(plan, "Implementation plan here");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "item1");
        assert!(!items[0].checked);
        assert_eq!(items[1].id, "item2");
        assert!(items[1].checked);

        // Test extract_plan function
        let extracted_plan = extract_plan(full_text);
        assert_eq!(extracted_plan, "Implementation plan here");

        // Test serialization
        let serialized = serialize_description_with_plan_and_checklist(&original, &plan, &items);
        assert!(serialized.contains("Task description"));
        assert!(serialized.contains("---PLAN---"));
        assert!(serialized.contains("Implementation plan here"));
        assert!(serialized.contains("---CHECKLIST---"));
        assert!(serialized.contains("- [ ] item1: First item"));
        assert!(serialized.contains("- [x] item2: Done item"));

        // Test round-trip
        let (original2, plan2, items2) = parse_description_with_plan_and_checklist(&serialized);
        assert_eq!(original, original2);
        assert_eq!(plan, plan2);
        assert_eq!(items.len(), items2.len());
        for (item1, item2) in items.iter().zip(items2.iter()) {
            assert_eq!(item1.id, item2.id);
            assert_eq!(item1.text, item2.text);
            assert_eq!(item1.checked, item2.checked);
        }
    }

    #[test]
    fn test_plan_replacement() {
        use crate::backend::{parse_description_with_plan_and_checklist, serialize_description_with_plan_and_checklist};
        
        // Test replacing an existing plan
        let old_full = "Task description\n---PLAN---\nOld plan\n---CHECKLIST---\n- [ ] item1: Item\n";
        let new_plan = "New implementation plan";
        let (desc, _, items) = parse_description_with_plan_and_checklist(old_full);
        
        let serialized = serialize_description_with_plan_and_checklist(&desc, &new_plan, &items);
        assert!(serialized.contains("New implementation plan"));
        assert!(!serialized.contains("Old plan"));
        assert!(serialized.contains("- [ ] item1: Item"));
        
        // Verify it parses correctly
        let (parsed_desc, parsed_plan, parsed_items) = parse_description_with_plan_and_checklist(&serialized);
        assert_eq!(parsed_desc, "Task description");
        assert_eq!(parsed_plan, "New implementation plan");
        assert_eq!(parsed_items.len(), 1);
    }
}
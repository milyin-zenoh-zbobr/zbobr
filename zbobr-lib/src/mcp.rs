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
    task::{Model, ChecklistItem, Parameter, Role, Stage, TaskSession, Tool},
    Zbobr,
};

// Instruction shared across all role prompts explaining branch isolation rules.
fn branch_isolation_instruction() -> String {
    format!(
        "Workspace branch isolation: When preparing the workspace, clone ONLY the destination branch as provided by the MCP tool `{}` and avoid fetching or using any other branches. Do NOT use branches other than the destination branch and the designated work branch obtained via `{}`. If you need temporary or experimental branches, prefix their names with the work branch name to avoid interfering with other agents.",
        planner_tools::GET_PARAM_DESTINATION_BRANCH,
        planner_tools::GET_PARAM_WORK_BRANCH,
    )
}

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
pub struct ShortNameParam {
    #[schemars(description = "Short name for the branch (e.g. 'implementation', 'fix-typo')")]
    pub short_name: String,
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

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct SetDestinationRepositoryParam {
    #[schemars(description = "Destination repository in owner/name format (or null to unset)")]
    pub value: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct SetDestinationBranchParam {
    #[schemars(description = "Work branch postfix (the final segment after prefix/task_id) (or null to unset)")]
    pub value: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct SetWorkBranchParam {
    #[schemars(description = "Work branch name (or null to unset)")]
    pub value: Option<String>,
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
    REPORT_ERROR = "report_error",
    PULL_WORK = "pull_work",
    GET_CHECKLIST = "get_checklist",
    GET_PARAM_DESTINATION_REPOSITORY = "get_param_destination_repository",
    SET_PARAM_DESTINATION_REPOSITORY = "set_param_destination_repository",
    SET_PARAM_DESTINATION_BRANCH = "set_param_destination_branch",
    GET_PARAM_DESTINATION_BRANCH = "get_param_destination_branch",
    SET_PARAM_WORK_BRANCH_POSTFIX = "set_param_work_branch_postfix",
    GET_PARAM_WORK_BRANCH = "get_param_work_branch",
}

mcp_tools! {
    worker_tools,
    GET_DESCRIPTION = "get_description",
    GET_DISCUSSION = "get_discussion",
    GET_PLAN = "get_plan",
    REPORT_ERROR = "report_error",
    ASK_USER = "ask_user",
    ASK_PLANNER = "ask_planner",
    PULL_WORK = "pull_work",
    PUSH_WORK = "push_work",
    GET_CHECKLIST = "get_checklist",
    INSERT_CHECKLIST_ITEM = "insert_checklist_item",
    UPDATE_CHECKLIST_ITEM = "update_checklist_item",
    CHECK_CHECKLIST_ITEM = "check_checklist_item",
    DELETE_CHECKLIST_ITEM = "delete_checklist_item",
    GET_PARAM_DESTINATION_BRANCH = "get_param_destination_branch",
    GET_PARAM_WORK_BRANCH = "get_param_work_branch",
}

mcp_tools! {
    reviewer_tools,
    GET_DESCRIPTION = "get_description",
    GET_DISCUSSION = "get_discussion",
    GET_PLAN = "get_plan",
    REPORT_ERROR = "report_error",
    PULL_WORK = "pull_work",
    GET_CHECKLIST = "get_checklist",
    INSERT_CHECKLIST_ITEM = "insert_checklist_item",
    UPDATE_CHECKLIST_ITEM = "update_checklist_item",
    DELETE_CHECKLIST_ITEM = "delete_checklist_item",
    GET_PARAM_DESTINATION_BRANCH = "get_param_destination_branch",
    GET_PARAM_WORK_BRANCH = "get_param_work_branch",
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

Investigate a task and create an implementation plan with actionable steps.

## Access Model

    You can access the internet and run local commands. Your restrictions:
    - Do NOT run git clone/pull/fetch — use `{pull_work}` instead
    - Use MCP `{post_plan}` to post the implementation plan
    - Use MCP `{report_error}` only to report technical errors; use `{ask_user}` to request the user's explanations related to the task
    - For reading GitHub data: use `git` and `gh` CLI only when no MCP tool provides the needed information
    - NEVER use git/gh for writing, pushing, or sending data to GitHub

## Workspace isolation

    {branch_isolation}

Work autonomously. Do not ask the user for anything.

## Workflow

1. Call `{get_description}` to read the user task description
2. Call `{get_plan}` to read an existing plan if there is one
3. Call `{get_discussion}` for context and prior comments and questions to existing plan
4. **Set task parameters** that will guide the implementation:
    - Call `{set_param_destination_repository}` with the target GitHub repository (owner/repo format, without branch name)
    - Call `{set_param_destination_branch}` (e.g., "main", "develop")
    - Call `{set_param_work_branch_postfix}` with the work branch postfix (e.g., "implement-feature") — the full work branch will be formed from prefix, task id and this postfix
    - Use `{get_param_destination_repository}`, `{get_param_destination_branch}`, `{get_param_work_branch}` to read current values
5. Pull the destination repository using `{pull_work}` to investigate the codebase, understand the context, and design the plan. This also ensures the repo is cached for the worker later.
6. Explore the codebase, identify and document the files, crates, modules, and keywords relevant to the task. These help define the scope and guide the worker:
   - List specific files that need to be modified or created
   - Identify crates/modules that contain related functionality
   - Include keywords/concepts the worker should focus on (e.g., "async/await", "error handling", "API compatibility")
   - This context narrows the worker's scope and prevents unnecessary exploration
7. Design a solution. 
8. Post a solution in the form of a text plan with `{post_plan}`. Use planning mode if available.
"#,
    branch_isolation = branch_isolation_instruction(),
        get_description = planner_tools::GET_DESCRIPTION,
        get_discussion = planner_tools::GET_DISCUSSION,
        get_plan = planner_tools::GET_PLAN,
        post_plan = planner_tools::POST_PLAN,
        report_error = planner_tools::REPORT_ERROR,
        ask_user = worker_tools::ASK_USER,
        pull_work = planner_tools::PULL_WORK,
        get_param_destination_repository = planner_tools::GET_PARAM_DESTINATION_REPOSITORY,
        set_param_destination_repository = planner_tools::SET_PARAM_DESTINATION_REPOSITORY,
        get_param_destination_branch = planner_tools::GET_PARAM_DESTINATION_BRANCH,
        set_param_destination_branch = planner_tools::SET_PARAM_DESTINATION_BRANCH,
        set_param_work_branch_postfix = planner_tools::SET_PARAM_WORK_BRANCH_POSTFIX,
        get_param_work_branch = planner_tools::GET_PARAM_WORK_BRANCH,
    )
}


/// Generate hardcoded worker instructions using tool name constants.
pub fn worker_instructions() -> String {
    format!(
        r#"# Worker Agent

Implement an approved plan by writing code and progressing checklist items.

## Checklist: Your Work Memory

The checklist is your persistent memory for this task. It survives across sessions and tells you exactly where to continue if the work is interrupted.

**Key principles:**
- Start by using `{get_checklist}` to read the current checklist — it tells you exactly where you are in the work.
- If the checklist is empty when you start, use `{insert_checklist_item}` to create it based on the plan. Break the plan into clear, actionable steps.
- Each checklist item should describe a meaningful unit of work (for example: "add unit tests for X", "refactor module Y", "update API to validate Z"). Do NOT use checklist items to record internal or platform tool actions (for example: "call {push_work}" or "run {pull_work}").
- Use `{check_checklist_item}` to mark items as checked (`✓`) when you complete them to record progress.
- Use `{insert_checklist_item}` to add new items during work if you discover additional steps needed.
- Use `{update_checklist_item}` to edit item text to refine understanding as you work.
- Use `{delete_checklist_item}` to remove items only if they become unnecessary (keep most items for history). **Note:** You cannot delete checked items—this prevents accidental loss of completed work history.

## Access Model

    You can access the internet and run local commands. Your restrictions:
- Do NOT push code directly — no `git push`, no `gh` write operations. The platform coordinates repository remote actions; do not include submission or remote-write actions as checklist items.
- Do NOT run git clone/pull/fetch directly for setting up work — platform tools can prepare the workspace when available. If you need repository data, use the provided helper tools rather than raw git commands.
- For reading GitHub data: use `git` and `gh` CLI only when no platform tool provides the needed information.
- NEVER use git/gh for writing, pushing, or sending data to GitHub.
- The work repository has remote information controlled by the platform; you must not perform direct remote writes yourself.
    - The work repository has remote information controlled by the platform; you must not perform direct remote writes yourself.

## Workspace isolation

    {branch_isolation}

Work autonomously. Do not ask the user for anything unless the task genuinely requires human input.

## Workflow

1. Call `{get_description}` to read the task
2. Call `{get_plan}` to retrieve the approved implementation plan (posted by the planner)
3. Call `{get_checklist}` to read the implementation steps
4. **If checklist is empty**: Create it using `{insert_checklist_item}` to break down the plan into clear, actionable steps (task-focused items only)
5. Call `{get_discussion}` if you need additional context from comments
6. **Focus on one unchecked checklist item during this session**. Assume checked items were completed in previous sessions. In exceptional cases where multiple items logically depend on the same setup and can be done together, you may do more than one, but this should be rare.
7. Use platform-provided workspace setup helper `{pull_work}` to prepare the repository and environment; when working with branches, consult `{get_param_destination_branch}` and `{get_param_work_branch}` for branch names if needed.
8. `cd` into the returned path and implement the plan
9. Commit changes locally with clear messages (describe what the change does, why, and reference relevant checklist item)
10. When implementation for an item is complete, mark the item done with `{check_checklist_item}`, save intermediate results with `{push_work}`, and update or insert follow-up items as needed
11. Do not add low-level platform or tool-invocation steps (for example, `{push_work}`) into your checklist — checklist items should remain human-meaningful and task-focused
12. If you need human clarification or intervention, call `{ask_user}` or `{ask_planner}` as appropriate; use `{report_error}` only to report technical errors"#,
        get_description = worker_tools::GET_DESCRIPTION,
        get_discussion = worker_tools::GET_DISCUSSION,
        get_plan = worker_tools::GET_PLAN,
        get_checklist = worker_tools::GET_CHECKLIST,
        insert_checklist_item = worker_tools::INSERT_CHECKLIST_ITEM,
        update_checklist_item = worker_tools::UPDATE_CHECKLIST_ITEM,
        check_checklist_item = worker_tools::CHECK_CHECKLIST_ITEM,
        delete_checklist_item = worker_tools::DELETE_CHECKLIST_ITEM,
        report_error = worker_tools::REPORT_ERROR,
        pull_work = worker_tools::PULL_WORK,
        get_param_destination_branch = worker_tools::GET_PARAM_DESTINATION_BRANCH,
        get_param_work_branch = worker_tools::GET_PARAM_WORK_BRANCH,
        push_work = worker_tools::PUSH_WORK,
        ask_user = worker_tools::ASK_USER,
        ask_planner = worker_tools::ASK_PLANNER,
        branch_isolation = branch_isolation_instruction(),
    )
}

/// Generate hardcoded reviewer instructions using tool name constants.
pub fn reviewer_instructions() -> String {
    format!(
        r#"# Reviewer Agent

Review the implementation changes and ensure they meet coding standards and task requirements.

## Checklist: Review Remarks

The checklist is shared with the worker. It contains both implementation work items and review remarks. Add your review findings as new checklist items to communicate issues that need fixing.

**How to use the checklist:**
- Start by using `{get_checklist}` to see what's there — both work items and any prior review remarks.
- Use `{insert_checklist_item}` to add each issue you find. Prefix with `[REVIEW]` to distinguish your remarks from work items.
- Use `{update_checklist_item}` to clarify or refine your remarks if needed.
- Use `{delete_checklist_item}` to remove remarks only if they become irrelevant. **Note:** You cannot delete checked items—only unchecked remarks can be removed.
- The worker will mark your review remarks as done — do not check items yourself.

## Access Model

You have read-only access to all task information:
- Use `{get_description}` to understand the original task
- Use `{get_plan}` to see the implementation plan
- Use `{get_discussion}` for context and prior comments
- Use `{pull_work}` to access the work repository and examine changes
    - Use `{ask_user}` to request the user's explanations related to review findings; use `{report_error}` only to report technical errors
- You can run local git commands to examine changes, but you cannot push

You can run local git commands to examine changes, but you cannot push

## Workspace isolation

    {branch_isolation}

Work autonomously. Do not ask the user for anything.

## Workflow

1. Call `{get_description}` to understand the task requirements
2. Call `{get_plan}` to see what was supposed to be implemented
3. Call `{get_discussion}` for additional context
4. Call `{get_checklist}` to see what's been done and what review remarks already exist
5. Set up the repository using `{pull_work}` to access the implementation
6. `cd` into the returned path
7. Use `{get_param_work_branch}` to get the work branch name
8. Use `{get_param_destination_branch}` to get the target branch name
9. Compare changes using git:
   - `git diff <destination_branch>..<work_branch>` to see all changes
   - `git log <destination_branch>..<work_branch>` to see commits
10. Review the changes for:
    - Conformance to the task requirements and plan
    - Code quality and style adherence
    - Proper error handling
    - Test coverage (if applicable)
    - Documentation completeness
    - Any potential bugs or issues
11. For each issue found:
    - Call `{insert_checklist_item}` to add a review remark (prefix with `[REVIEW]`)
    - Include specific file names, line numbers, and what needs to be fixed
"#,
        get_description = reviewer_tools::GET_DESCRIPTION,
        get_plan = reviewer_tools::GET_PLAN,
        get_discussion = reviewer_tools::GET_DISCUSSION,
        get_checklist = reviewer_tools::GET_CHECKLIST,
        insert_checklist_item = reviewer_tools::INSERT_CHECKLIST_ITEM,
        update_checklist_item = reviewer_tools::UPDATE_CHECKLIST_ITEM,
        delete_checklist_item = reviewer_tools::DELETE_CHECKLIST_ITEM,
        report_error = reviewer_tools::REPORT_ERROR,
        pull_work = reviewer_tools::PULL_WORK,
        ask_user = worker_tools::ASK_USER,
        get_param_destination_branch = reviewer_tools::GET_PARAM_DESTINATION_BRANCH,
        get_param_work_branch = reviewer_tools::GET_PARAM_WORK_BRANCH,
        branch_isolation = branch_isolation_instruction(),
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
#[allow(async_fn_in_trait)]
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

    async fn report_error_impl(&self, message: &str) -> String {
        tracing::info!("[{}#{}] report_error", self.role_name(), self.session().task_id());
        let hostname = get_hostname();

        if let Err(e) = self.session().post_message(message, "error", &hostname).await {
            tracing::error!("Failed to post error message for task {}: {e}", self.session().task_id());
            return format!("Error posting error message: {e}");
        }

        // Signal to pause task processing and wait for user response
        if let Err(e) = self.session().set_signal(crate::Signal::GoAsk).await {
            tracing::error!("Failed to set signal GoAsk for task {} after reporting error: {e}", self.session().task_id());
            return format!("Error reporting error but error pausing task: {e}");
        }

        "Error reported to user - task paused pending response".to_string()
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
        let item_id = id.to_string();
        match self.session().modify_task(|task| {
            if let Some(item) = task.checklist.iter_mut().find(|item| item.id == item_id) {
                item.checked = checked;
            }
        }).await {
            Ok(()) => {
                // Checklist item state updated; signal transitions are handled by
                // the main/run loop after a role session completes. Do not set
                // task signal here to avoid racing state transitions.
                format!("Checklist item '{}' checked state updated to {}", id, checked)
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn insert_checklist_item_impl(&self, id: &str, after_id: Option<String>, text: &str) -> String {
        tracing::info!("[{}#{}] insert_checklist_item id={} after_id={:?}", self.role_name(), self.session().task_id(), id, after_id);
        let item_id = id.to_string();
        let item_text = text.to_string();
        let after = after_id.clone();
        
        // Validate first by reading the task
        match self.session().get_checklist().await {
            Ok(items) => {
                if items.iter().any(|item| item.id == item_id) {
                    return format!("Error: Checklist item with id '{}' already exists", id);
                }
                if let Some(ref aid) = after {
                    if !items.iter().any(|item| item.id == *aid) {
                        return format!("Error: Checklist item with id '{}' not found", aid);
                    }
                }
            }
            Err(e) => return format!("Error: {e}"),
        }

        match self.session().modify_task(|task| {
            let new_item = ChecklistItem {
                id: item_id,
                checked: false,
                text: item_text,
            };

            if let Some(ref after_id) = after {
                if let Some(pos) = task.checklist.iter().position(|item| item.id == *after_id) {
                    task.checklist.insert(pos + 1, new_item);
                } else {
                    task.checklist.push(new_item);
                }
            } else {
                task.checklist.push(new_item);
            }
        }).await {
            Ok(()) => format!("Checklist item '{}' inserted", id),
            Err(e) => format!("Error updating task: {e}"),
        }
    }

    async fn update_checklist_item_impl(&self, id: &str, text: &str) -> String {
        tracing::info!("[{}#{}] update_checklist_item id={}", self.role_name(), self.session().task_id(), id);
        let item_id = id.to_string();
        let item_text = text.to_string();
        match self.session().modify_task(|task| {
            if let Some(item) = task.checklist.iter_mut().find(|item| item.id == item_id) {
                item.text = item_text;
            }
        }).await {
            Ok(()) => format!("Checklist item '{}' updated", id),
            Err(e) => format!("Error updating task: {e}"),
        }
    }

    async fn delete_checklist_item_impl(&self, id: &str) -> String {
        tracing::info!("[{}#{}] delete_checklist_item id={}", self.role_name(), self.session().task_id(), id);
        let item_id = id.to_string();

        // Pre-validate: check the item exists and is not checked
        match self.session().get_checklist().await {
            Ok(items) => {
                if let Some(item) = items.iter().find(|i| i.id == item_id) {
                    if item.checked {
                        return format!("Error: Cannot delete checked checklist item '{}'. Checked items are preserved as work history.", id);
                    }
                } else {
                    return format!("Error: Checklist item with id '{}' not found", id);
                }
            }
            Err(e) => return format!("Error: {e}"),
        }

        match self.session().modify_task(|task| {
            task.checklist.retain(|item| item.id != item_id);
        }).await {
            Ok(()) => format!("Checklist item '{}' deleted", id),
            Err(e) => format!("Error updating task: {e}"),
        }
    }
}

/// Planner-specific MCP implementations
#[allow(async_fn_in_trait)]
pub trait PlannerMcpImpl: CommonMcpImpl {
    async fn get_param_impl(&self, param: Parameter) -> String {
        let param_name = param.name();
        tracing::info!("[planner#{}] get_param_{}", self.session().task_id(), param_name);
        match self.session().get_parameter(param_name).await {
            Ok(Some(value)) => value,
            Ok(None) => format!("{} is not set", param_name),
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn set_param_impl(&self, param: Parameter, value: Option<String>) -> String {
        let param_name = param.name();
        tracing::info!("[planner#{}] set_param_{} value={:?}", self.session().task_id(), param_name, value);
        match self.session().set_parameter(param_name, value).await {
            Ok(()) => format!("{} updated", param_name),
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn post_plan_impl(&self, plan: &str) -> String {
        tracing::info!("[planner#{}] post_plan", self.session().task_id());
        let plan_text = plan.to_string();
        match self.session().modify_task(|task| {
            task.plan = plan_text;
        }).await {
            Ok(()) => {
                // Mark plan as ready for worker to implement
                if let Err(e) = self.session().set_signal(crate::Signal::GoWork).await {
                    tracing::error!("Failed to set signal GoWork for task {} after posting plan: {e}", self.session().task_id());
                    return format!("Plan posted but error marking task ready for work: {e}");
                }
                "Plan posted and task ready for worker implementation".to_string()
            }
            Err(e) => format!("Error updating task: {e}"),
        }
    }

    async fn pull_work_impl(&self) -> String {
        tracing::info!("[planner#{}] pull_work", self.session().task_id());
        match self.session().pull_work().await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn get_param_destination_repository_impl(&self) -> String {
        self.get_param_impl(Parameter::DestinationRepository).await
    }

    async fn set_param_destination_repository_impl(&self, value: Option<String>) -> String {
        self.set_param_impl(Parameter::DestinationRepository, value).await
    }

    async fn get_param_destination_branch_impl(&self) -> String {
        self.get_param_impl(Parameter::DestinationBranch).await
    }

    async fn set_param_destination_branch_impl(&self, value: Option<String>) -> String {
        self.set_param_impl(Parameter::DestinationBranch, value).await
    }

    async fn set_param_work_branch_postfix_impl(&self, value: Option<String>) -> String {
        tracing::info!("[planner#{}] set_param_work_branch_postfix value={:?}", self.session().task_id(), value);
        match value {
            Some(postfix) => {
                let full = self.session().create_branch_name(&postfix);
                self.set_param_impl(Parameter::WorkBranch, Some(full)).await
            }
            None => {
                // Unset work branch parameter
                self.set_param_impl(Parameter::WorkBranch, None).await
            }
        }
    }

    async fn get_param_work_branch_impl(&self) -> String {
        self.get_param_impl(Parameter::WorkBranch).await
    }

}

/// Worker-specific MCP implementations
#[allow(async_fn_in_trait)]
pub trait WorkerMcpImpl: CommonMcpImpl {
    async fn get_param_impl(&self, param: Parameter) -> String {
        let param_name = param.name();
        tracing::info!("[worker#{}] get_param_{}", self.session().task_id(), param_name);
        match self.session().get_parameter(param_name).await {
            Ok(Some(value)) => value,
            Ok(None) => format!("{} is not set", param_name),
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn get_param_destination_branch_impl(&self) -> String {
        self.get_param_impl(Parameter::DestinationBranch).await
    }

    async fn get_param_work_branch_impl(&self) -> String {
        self.get_param_impl(Parameter::WorkBranch).await
    }

    async fn ask_user_impl(&self, message: &str) -> String {
        tracing::info!("[worker#{}] ask_user", self.session().task_id());
        let hostname = get_hostname();
        
        if let Err(e) = self.session().post_message(message, self.role().as_str(), &hostname).await {
            tracing::error!("Failed to post worker message for task {}: {e}", self.session().task_id());
            return format!("Error posting message: {e}");
        }
        
        // Signal to pause task processing and wait for user response
        if let Err(e) = self.session().set_signal(crate::Signal::GoAsk).await {
            tracing::error!("Failed to set signal GoAsk for task {} after ask_user: {e}", self.session().task_id());
            return format!("Question posted but error pausing task: {e}");
        }
        "Message posted to user - task paused pending response".to_string()
    }

    async fn ask_planner_impl(&self, message: &str) -> String {
        tracing::info!("[worker#{}] ask_planner", self.session().task_id());
        let hostname = get_hostname();
        
        if let Err(e) = self.session().post_message(message, self.role().as_str(), &hostname).await {
            tracing::error!("Failed to post worker->planner message for task {}: {e}", self.session().task_id());
            return format!("Error posting message: {e}");
        }
        
        // Pass task back to planner agent for clarification or re-planning
        if let Err(e) = self.session().set_signal(crate::Signal::GoPlan).await {
            tracing::error!("Failed to set signal GoPlan for task {} after ask_planner: {e}", self.session().task_id());
            return format!("Message posted but error returning to planner: {e}");
        }
        "Message posted to planner - task returned for clarification".to_string()
    }


    async fn pull_work_impl(&self) -> String {
        tracing::info!("[worker#{}] pull_work", self.session().task_id());
        match self.session().pull_work().await {
            Ok(path) => path,
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn push_work_impl(&self) -> String {
        tracing::info!("[worker#{}] push_work", self.session().task_id());
        match self.session().push_work().await {
            Ok(()) => "Work branch pushed successfully".to_string(),
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

/// Reviewer-specific MCP implementations
#[allow(async_fn_in_trait)]
pub trait ReviewerMcpImpl: CommonMcpImpl {
    async fn get_param_impl(&self, param: Parameter) -> String {
        let param_name = param.name();
        tracing::info!("[reviewer#{}] get_param_{}", self.session().task_id(), param_name);
        match self.session().get_parameter(param_name).await {
            Ok(Some(value)) => value,
            Ok(None) => format!("{} is not set", param_name),
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn get_param_destination_branch_impl(&self) -> String {
        self.get_param_impl(Parameter::DestinationBranch).await
    }

    async fn get_param_work_branch_impl(&self) -> String {
        self.get_param_impl(Parameter::WorkBranch).await
    }

    async fn pull_work_impl(&self) -> String {
        tracing::info!("[reviewer#{}] pull_work", self.session().task_id());
        match self.session().pull_work().await {
            Ok(path) => path,
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

    #[tool(description = "Report an error to the user and pause task processing")]
    async fn report_error(&self, Parameters(params): Parameters<MessageParam>) -> String {
        self.report_error_impl(&params.message).await
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
        description = "Clone the fork of the destination_repository and return the path. Automatically sets the current branch to work_branch (created from destination_branch). Stashes local changes if a different branch is selected as current. The work repository has all remote information cleared - only pull_work and push_work know where to push/pull. The model must not do git push directly."
    )]
    async fn pull_work(&self) -> String {
        self.pull_work_impl().await
    }

    #[tool(description = "Get the task checklist as a list of checkbox items")]
    async fn get_checklist(&self) -> String {
        self.get_checklist_impl().await
    }

    #[tool(description = "Get the destination repository URL for this task (read-only)")]
    async fn get_param_destination_repository(&self) -> String {
        self.get_param_destination_repository_impl().await
    }

    #[tool(description = "Set the destination repository URL for this task (e.g. 'owner/repo')")]
    async fn set_param_destination_repository(&self, Parameters(params): Parameters<SetDestinationRepositoryParam>) -> String {
        self.set_param_destination_repository_impl(params.value).await
    }

    #[tool(description = "Get the destination branch name for this task (read-only)")]
    async fn get_param_destination_branch(&self) -> String {
        self.get_param_destination_branch_impl().await
    }

    #[tool(description = "Set the destination branch name for this task (e.g. 'main')")]
    async fn set_param_destination_branch(&self, Parameters(params): Parameters<SetDestinationBranchParam>) -> String {
        self.set_param_destination_branch_impl(params.value).await
    }

    #[tool(description = "Set the work branch postfix for this task (the postfix segment, e.g. 'implement-feature')")]
    async fn set_param_work_branch_postfix(&self, Parameters(params): Parameters<SetDestinationBranchParam>) -> String {
        self.set_param_work_branch_postfix_impl(params.value).await
    }

    #[tool(description = "Get the work branch name for this task (read-only)")]
    async fn get_param_work_branch(&self) -> String {
        self.get_param_work_branch_impl().await
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

    #[tool(description = "Report an error to the user and pause task processing")]
    async fn report_error(&self, Parameters(params): Parameters<MessageParam>) -> String {
        self.report_error_impl(&params.message).await
    }

    #[tool(description = "Post a message to the user and pause task processing until user responds")]
    async fn ask_user(&self, Parameters(params): Parameters<MessageParam>) -> String {
        self.ask_user_impl(&params.message).await
    }

    #[tool(description = "Post a message to the planner and pass the task back for clarification or re-planning")]
    async fn ask_planner(&self, Parameters(params): Parameters<MessageParam>) -> String {
        self.ask_planner_impl(&params.message).await
    }

    #[tool(description = "Get the current implementation plan for this task")]
    async fn get_plan(&self) -> String {
        self.get_plan_impl().await
    }

    #[tool(
        description = "Clone the fork of the destination_repository and return the path. Automatically sets the current branch to work_branch (created from destination_branch). Stashes local changes if a different branch is selected as current. The work repository has all remote information cleared - only pull_work and push_work know where to push/pull. The model must not do git push directly."
    )]
    async fn pull_work(&self) -> String {
        self.pull_work_impl().await
    }

    #[tool(
        description = "Push the work_branch in the cloned repository. Returns nothing. Stashes local changes if a different branch is selected as current. The work repository has all remote information cleared - only pull_work and push_work know where to push/pull. The model must not do git push directly."
    )]
    async fn push_work(&self) -> String {
        self.push_work_impl().await
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

    #[tool(description = "Delete an unchecked checklist item (checked items are preserved as history)")]
    async fn delete_checklist_item(&self, Parameters(params): Parameters<DeleteChecklistItemParam>) -> String {
        self.delete_checklist_item_impl(&params.id).await
    }

    #[tool(description = "Get the destination branch name for this task (read-only)")]
    async fn get_param_destination_branch(&self) -> String {
        self.get_param_destination_branch_impl().await
    }

    #[tool(description = "Get the work branch name for this task (read-only)")]
    async fn get_param_work_branch(&self) -> String {
        self.get_param_work_branch_impl().await
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

// -- Reviewer MCP service --

#[derive(Clone)]
pub struct ReviewerMcp {
    session: TaskSession,
    tool_router: ToolRouter<Self>,
}

impl CommonMcpImpl for ReviewerMcp {
    fn session(&self) -> &TaskSession {
        &self.session
    }
    
    fn role(&self) -> Role {
        Role::Reviewer
    }
}

impl ReviewerMcpImpl for ReviewerMcp {}

#[tool_router]
impl ReviewerMcp {
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

    #[tool(description = "Report an error to the user and pause task processing")]
    async fn report_error(&self, Parameters(params): Parameters<MessageParam>) -> String {
        self.report_error_impl(&params.message).await
    }

    #[tool(description = "Get the current implementation plan for this task (read-only)")]
    async fn get_plan(&self) -> String {
        self.get_plan_impl().await
    }

    #[tool(
        description = "Clone the fork of the destination_repository and return the path. Automatically sets the current branch to work_branch (created from destination_branch). Read-only access for review purposes."
    )]
    async fn pull_work(&self) -> String {
        self.pull_work_impl().await
    }

    #[tool(description = "Get the task checklist as a list of checkbox items")]
    async fn get_checklist(&self) -> String {
        self.get_checklist_impl().await
    }

    #[tool(description = "Insert a new checklist item for review remarks (always created in unchecked state)")]
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

    #[tool(description = "Delete an unchecked checklist item (checked items are preserved as history)")]
    async fn delete_checklist_item(&self, Parameters(params): Parameters<DeleteChecklistItemParam>) -> String {
        self.delete_checklist_item_impl(&params.id).await
    }

    #[tool(description = "Get the destination branch name for this task (read-only)")]
    async fn get_param_destination_branch(&self) -> String {
        self.get_param_destination_branch_impl().await
    }

    #[tool(description = "Get the work branch name for this task (read-only)")]
    async fn get_param_work_branch(&self) -> String {
        self.get_param_work_branch_impl().await
    }
}

#[tool_handler]
impl ServerHandler for ReviewerMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "Reviewer tools: review implementation changes, add review remarks to checklist."
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

impl ReviewerMcp {
    /// Generate API documentation for reviewer tools
    pub fn generate_api_docs() -> String {
        let tools = Self::tool_router();
        generate_api_docs_from_router(&tools, "Reviewer")
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
        Role::Reviewer => {
            tracing::info!("Creating ReviewerMcp service for task {task_id} at path {path}");
            let svc = StreamableHttpService::new(
                move || {
                    tracing::debug!("Creating new ReviewerMcp instance for task {task_id}");
                    Ok(ReviewerMcp::new(zbobr.clone(), task_id))
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
            task_repo: "test/repo".to_string(),
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
            reviewer_prompts: vec![],
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
            task_repo: "test/repo".to_string(),
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
            reviewer_prompts: vec![],
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
            task_repo: "test/repo".to_string(),
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
            reviewer_prompts: vec![],
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

        // Test parsing with CRLF line endings
        let full_text_crlf = "Task description\r\n---PLAN---\r\nImplementation plan here\r\n---CHECKLIST---\r\n- [ ] item1: First item\r\n- [x] item2: Done item\r\n";
        let (original_crlf, plan_crlf, items_crlf) = parse_description_with_plan_and_checklist(full_text_crlf);
        assert_eq!(original_crlf, "Task description");
        assert_eq!(plan_crlf, "Implementation plan here");
        assert_eq!(items_crlf.len(), 2);

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
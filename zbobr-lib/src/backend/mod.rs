pub mod github;
pub mod stub;

use std::path::PathBuf;

use async_trait::async_trait;

use crate::{Model, Signal, Stage, Task, Tool, ZbobrError};
use crate::task::ChecklistItem;

// -- Plan and Checklist parsing and serialization helpers --

const PLAN_SEPARATOR: &str = "\n---PLAN---\n";
const CHECKLIST_SEPARATOR: &str = "\n---CHECKLIST---\n";

/// Parse a task description into (description, plan, checklist).
/// Format: description | ---PLAN--- | plan text | ---CHECKLIST--- | checklist
pub fn parse_description_with_plan_and_checklist(full_text: &str) -> (String, String, Vec<ChecklistItem>) {
    // First split by checklist
    let parts: Vec<&str> = full_text.split(CHECKLIST_SEPARATOR).collect();
    
    let (desc_and_plan, checklist_text) = match parts.len() {
        1 => (parts[0], ""),
        _ => (parts[0], parts[1]),
    };
    
    // Now split desc_and_plan by plan separator
    let plan_parts: Vec<&str> = desc_and_plan.split(PLAN_SEPARATOR).collect();
    let (description, plan) = match plan_parts.len() {
        1 => (plan_parts[0].to_string(), String::new()),
        _ => (plan_parts[0].to_string(), plan_parts[1].trim().to_string()),
    };
    
    // Parse checklist items
    let mut items = Vec::new();
    for line in checklist_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        // Parse checkbox format: - [ ] id: text or - [x] id: text
        if let Some(rest) = line.strip_prefix("- [") {
            if let Some(pos) = rest.find(']') {
                let checkbox = &rest[..pos];
                let checked = checkbox.trim() == "x" || checkbox.trim() == "X";
                
                let after_checkbox = rest[pos + 1..].trim();
                if let Some(colon_pos) = after_checkbox.find(':') {
                    let id = after_checkbox[..colon_pos].trim().to_string();
                    let text = after_checkbox[colon_pos + 1..].trim().to_string();
                    
                    items.push(ChecklistItem { id, checked, text });
                }
            }
        }
    }
    
    (description, plan, items)
}

/// Parse a task description, separating the original description from the checklist.
/// Returns (original_description, checklist_items).
/// This function now also strips the plan section automatically.
pub fn parse_description_with_checklist(description: &str) -> (String, Vec<ChecklistItem>) {
    let (desc, _, items) = parse_description_with_plan_and_checklist(description);
    (desc, items)
}

/// Extract the original description, removing any existing plan and checklist sections.
/// This ensures no duplicate separators in the description.
pub fn strip_plan_and_checklist_from_description(description: &str) -> String {
    let (original, _, _) = parse_description_with_plan_and_checklist(description);
    original
}

/// Extract the original description, removing any existing checklist section.
/// This ensures no duplicate checklist separators in the description.
pub fn strip_checklist_from_description(description: &str) -> String {
    let (original, _) = parse_description_with_checklist(description);
    original
}

/// Extract the plan from a full description text.
/// Returns an empty string if no plan section exists.
pub fn extract_plan(full_text: &str) -> String {
    let (_, plan, _) = parse_description_with_plan_and_checklist(full_text);
    plan
}

/// Serialize description, plan and checklist items back into the full format.
/// Format: description | ---PLAN--- | plan | ---CHECKLIST--- | checklist
pub fn serialize_description_with_plan_and_checklist(
    original_description: &str,
    plan: &str,
    items: &[ChecklistItem],
) -> String {
    // Strip both plan and checklist from the description first
    let clean_description = strip_plan_and_checklist_from_description(original_description);
    
    let mut result = clean_description;
    
    // Add plan if present
    if !plan.is_empty() {
        result.push_str(PLAN_SEPARATOR);
        result.push_str(plan);
    }
    
    // Add checklist if present
    if !items.is_empty() {
        result.push_str(CHECKLIST_SEPARATOR);
        for item in items {
            let checkbox = if item.checked { "x" } else { " " };
            result.push_str(&format!("- [{}] {}: {}\n", checkbox, item.id, item.text));
        }
    }
    
    result
}

/// Serialize checklist items back into the full description format.
/// If the description contains an existing checklist, it will be replaced with the new one.
pub fn serialize_description_with_checklist(original_description: &str, items: &[ChecklistItem]) -> String {
    // Keep any existing plan, only replace checklist
    let current_plan = extract_plan(original_description);
    serialize_description_with_plan_and_checklist(original_description, &current_plan, items)
}

#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait Backend: Send + Sync {
    /// Get a task by ID.
    async fn get_task(&self, id: u64) -> Result<Task, ZbobrError>;

    /// Create a new task. Returns the task ID.
    #[allow(clippy::too_many_arguments)]
    async fn create_task(
        &self,
        title: &str,
        description: &str,
        stage: Stage,
        tool: Option<Tool>,
        model: Option<Model>,
        parent_task_id: Option<u64>,
        destination_repo: Option<String>,
        destination_branch: Option<String>,
    ) -> Result<u64, ZbobrError>;

    /// Close a task.
    async fn close_task(&self, id: u64) -> Result<(), ZbobrError>;

    /// Get all comments on a task as formatted discussion.
    async fn get_task_comments(&self, id: u64) -> Result<Vec<String>, ZbobrError>;

    /// Post a comment on a task with role and hostname metadata.
    async fn post_task_comment(
        &self,
        id: u64,
        body: &str,
        role: &str,
        hostname: &str,
    ) -> Result<(), ZbobrError>;

    /// Set the stage on a task by stage name.
    async fn set_task_stage(&self, id: u64, stage_name: &str) -> Result<(), ZbobrError>;

    /// Set or clear a signal on a task.
    async fn set_task_signal(&self, id: u64, signal: Option<Signal>) -> Result<(), ZbobrError>;

    /// Update the task description.
    async fn update_task_description(&self, id: u64, description: &str) -> Result<(), ZbobrError>;

    /// List open tasks with a given stage name, optionally filtered by tool.
    async fn list_tasks_by_stage(
        &self,
        stage_name: &str,
        tool: Option<Tool>,
    ) -> Result<Vec<Task>, ZbobrError>;

    /// Check if a task is closed.
    async fn is_task_closed(&self, id: u64) -> Result<bool, ZbobrError>;

    /// Check if a file exists in the domain repo.
    async fn repo_file_exists(&self, path: &str) -> Result<bool, ZbobrError>;

    /// Create or update a file in the domain repo.
    async fn create_repo_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<(), ZbobrError>;

    /// Ensure the domain repo exists.
    async fn ensure_domain_repo_exists(&self) -> Result<(), ZbobrError>;

    /// Clone a repo into the workspace, checkout specific branch, set up fork remote.
    /// Returns the local path.
    async fn clone_and_setup(
        &self,
        target_repo: &str,
        branch: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError>;

    /// Clone a repo and checkout specific branch for read-only investigation (no fork).
    async fn clone_readonly(
        &self,
        target_repo: &str,
        branch: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError>;

    /// Parse PR reference (URL or owner/repo#123) to (repo, branch).
    async fn parse_pr_to_repo_branch(&self, pr_ref: &str) -> Result<(String, String), ZbobrError>;

    /// Push the current branch to the fork remote and create a PR.
    async fn push_and_create_pr(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError>;

    // -- Setup methods --

    /// List all stages (milestones) in the domain repo.
    async fn list_stages(&self) -> Result<Vec<(u64, String)>, ZbobrError>;

    /// Create a stage (milestone) in the domain repo.
    async fn create_stage(&self, title: &str, description: &str) -> Result<(), ZbobrError>;

    /// Delete a stage by its number.
    async fn delete_stage(&self, number: u64) -> Result<(), ZbobrError>;

    /// List all labels in the domain repo.
    async fn list_labels(&self) -> Result<Vec<String>, ZbobrError>;

    /// Create a label in the domain repo.
    async fn create_label(
        &self,
        name: &str,
        color: &str,
        description: &str,
    ) -> Result<(), ZbobrError>;

    /// Initialize the domain repository with stages and labels.
    /// If force is true, overwrites existing labels.
    async fn setup_repository(&self, force: bool) -> Result<(), ZbobrError>;

    /// Validate that the backend can reach required resources (fork owner, domain repo, etc.).
    async fn validate_connectivity(&self) -> Result<(), ZbobrError>;

    /// Return a debug string of the backend state.
    fn debug_state(&self) -> String;
}

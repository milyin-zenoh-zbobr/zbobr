pub mod github;
pub mod stub;

use std::path::PathBuf;

use async_trait::async_trait;

use crate::{Label, Model, Stage, Task, Tool, ZbobrError};
use crate::task::PlanItem;

// -- Plan parsing and serialization helpers --

const PLAN_SEPARATOR: &str = "\n---PLAN---\n";

/// Parse a task description, separating the original description from the plan.
/// Returns (original_description, plan_items).
pub fn parse_description_with_plan(description: &str) -> (String, Vec<PlanItem>) {
    let parts: Vec<&str> = description.split(PLAN_SEPARATOR).collect();
    if parts.len() < 2 {
        // No plan section
        return (description.to_string(), Vec::new());
    }

    let original = parts[0].to_string();
    let plan_text = parts[1];
    
    let mut items = Vec::new();
    for line in plan_text.lines() {
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
                    
                    items.push(PlanItem { id, checked, text });
                }
            }
        }
    }
    
    (original, items)
}

/// Serialize plan items back into the full description format.
pub fn serialize_description_with_plan(original_description: &str, items: &[PlanItem]) -> String {
    if items.is_empty() {
        return original_description.to_string();
    }
    
    let mut result = original_description.to_string();
    result.push_str(PLAN_SEPARATOR);
    
    for item in items {
        let checkbox = if item.checked { "x" } else { " " };
        result.push_str(&format!("- [{}] {}: {}\n", checkbox, item.id, item.text));
    }
    
    result
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

    /// Add an arbitrary label to a task.
    async fn add_task_label(&self, id: u64, label: Label) -> Result<(), ZbobrError>;

    /// Remove a label from a task.
    async fn remove_task_label(&self, id: u64, label: Label) -> Result<(), ZbobrError>;

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

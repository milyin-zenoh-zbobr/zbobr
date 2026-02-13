pub mod github;
pub mod stub;

use std::path::PathBuf;

use async_trait::async_trait;

use crate::{Model, Parameter, Signal, Stage, Task, Tool, ZbobrError};
use crate::task::ChecklistItem;

// -- Plan and Checklist parsing and serialization helpers --

const PARAMETERS_SEPARATOR: &str = "\n---PARAMETERS---\n";
const PLAN_SEPARATOR: &str = "\n---PLAN---\n";
const CHECKLIST_SEPARATOR: &str = "\n---CHECKLIST---\n";

/// Parse parameters from the PARAMETERS section.
/// Returns a map of parameter names to values.
pub fn parse_parameters(params_text: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    for line in params_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim().to_string();
            let value = line[colon_pos + 1..].trim().to_string();
            params.insert(key, value);
        }
    }
    params
}

/// Serialize parameters into the PARAMETERS section format.
pub fn serialize_parameters(params: &std::collections::HashMap<String, String>) -> String {
    let mut result = String::new();
    for (key, value) in params {
        result.push_str(&format!("{}: {}\n", key, value));
    }
    result
}

/// Parse a task description into (description, parameters, plan, checklist).
/// Format: description | ---PARAMETERS--- | params | ---PLAN--- | plan text | ---CHECKLIST--- | checklist
pub fn parse_description_full(full_text: &str) -> (String, std::collections::HashMap<String, String>, String, Vec<ChecklistItem>) {
    // Normalize line endings so separators match regardless of \r\n vs \n.
    let normalized = if full_text.contains("\r\n") {
        full_text.replace("\r\n", "\n")
    } else {
        full_text.to_string()
    };

    // First split by checklist
    let parts: Vec<&str> = normalized.split(CHECKLIST_SEPARATOR).collect();
    
    let (before_checklist, checklist_text) = match parts.len() {
        1 => (parts[0], ""),
        _ => (parts[0], parts[1]),
    };
    
    // Now split by plan separator
    let plan_parts: Vec<&str> = before_checklist.split(PLAN_SEPARATOR).collect();
    let (before_plan, plan) = match plan_parts.len() {
        1 => (plan_parts[0], ""),
        _ => (plan_parts[0], plan_parts[1].trim()),
    };
    
    // Now split by parameters separator
    let param_parts: Vec<&str> = before_plan.split(PARAMETERS_SEPARATOR).collect();
    let (description, params_text) = match param_parts.len() {
        1 => (param_parts[0].to_string(), ""),
        _ => (param_parts[0].to_string(), param_parts[1].trim()),
    };
    
    // Parse parameters
    let parameters = parse_parameters(params_text);
    
    // Parse checklist items
    let mut items = Vec::new();
    for line in checklist_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        // Parse checkbox format: - [ ] id: text or - [x] id: text
        if let Some(rest) = line.strip_prefix("- [")
            && let Some(pos) = rest.find(']')
        {
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
    
    (description, parameters, plan.to_string(), items)
}

/// Parse a task description into (description, plan, checklist) - backward compatibility.
/// Format: description | ---PLAN--- | plan text | ---CHECKLIST--- | checklist
pub fn parse_description_with_plan_and_checklist(full_text: &str) -> (String, String, Vec<ChecklistItem>) {
    let (description, _parameters, plan, items) = parse_description_full(full_text);
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
    let (_, _, plan, _) = parse_description_full(full_text);
    plan
}

/// Extract parameters from a full description text.
pub fn extract_parameters(full_text: &str) -> std::collections::HashMap<String, String> {
    let (_, params, _, _) = parse_description_full(full_text);
    params
}

/// Serialize description, parameters, plan and checklist items back into the full format.
/// Format: description | ---PARAMETERS--- | params | ---PLAN--- | plan | ---CHECKLIST--- | checklist
pub fn serialize_description_full(
    original_description: &str,
    parameters: &std::collections::HashMap<String, String>,
    plan: &str,
    items: &[ChecklistItem],
) -> String {
    // Strip everything from the description first
    let (clean_description, _, _, _) = parse_description_full(original_description);
    
    let mut result = clean_description;
    
    // Add parameters if present
    if !parameters.is_empty() {
        result.push_str(PARAMETERS_SEPARATOR);
        result.push_str(&serialize_parameters(parameters));
    }
    
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

/// Serialize description, plan and checklist items back into the full format.
/// Format: description | ---PARAMETERS--- | params | ---PLAN--- | plan | ---CHECKLIST--- | checklist
/// Preserves any existing parameters.
pub fn serialize_description_with_plan_and_checklist(
    original_description: &str,
    plan: &str,
    items: &[ChecklistItem],
) -> String {
    // Preserve existing parameters
    let parameters = extract_parameters(original_description);
    serialize_description_full(original_description, &parameters, plan, items)
}

/// Serialize checklist items back into the full description format.
/// If the description contains an existing checklist, it will be replaced with the new one.
pub fn serialize_description_with_checklist(original_description: &str, items: &[ChecklistItem]) -> String {
    // Keep any existing plan, only replace checklist
    let current_plan = extract_plan(original_description);
    serialize_description_with_plan_and_checklist(original_description, &current_plan, items)
}

/// Merge concurrent updates to a task description.
/// 
/// This function handles the case where two concurrent updates have been made to different
/// sections of the task description (description, parameters, plan, checklist).
/// 
/// Given:
/// - `original`: The description as it was when we first read it
/// - `current`: The description as it exists now (after someone else modified it)
/// - `our_new`: The description we want to write
/// 
/// This function extracts what parts we modified vs what parts someone else modified,
/// and merges them intelligently:
/// - If we both modified the same section, our change wins (last write wins, simplified)
/// - If only one of us modified a section, that modification is preserved
/// 
/// The strategy is to parse all three versions, detect what changed in each,
/// and prefer newer values while preserving non-conflicting changes.
pub fn merge_concurrent_description_updates(
    original: &str,
    current: &str,
    our_new: &str,
) -> String {
    // Parse all three versions
    let (orig_desc, orig_params, orig_plan, orig_checklist) = parse_description_full(original);
    let (curr_desc, curr_params, curr_plan, curr_checklist) = parse_description_full(current);
    let (new_desc, new_params, new_plan, new_checklist) = parse_description_full(our_new);

    // Determine what we changed
    let we_changed_desc = new_desc != orig_desc;
    let we_changed_params = new_params != orig_params;
    let we_changed_plan = new_plan != orig_plan;
    let we_changed_checklist = serde_json::to_string(&new_checklist).unwrap_or_default() 
        != serde_json::to_string(&orig_checklist).unwrap_or_default();

    // Merge: prefer our changes if we made them, otherwise prefer their changes
    let merged_desc = if we_changed_desc { new_desc } else { curr_desc };
    let merged_params = if we_changed_params { new_params } else { curr_params };
    let merged_plan = if we_changed_plan { new_plan } else { curr_plan };
    let merged_checklist = if we_changed_checklist { new_checklist } else { curr_checklist };

    // Serialize back with the merged content
    serialize_description_full(
        &merged_desc,
        &merged_params,
        &merged_plan,
        &merged_checklist,
    )
}

/// Create a placeholder file in a branch to ensure it has at least one commit.
/// This is used to prevent GitHub PR API from rejecting branches with no commits.
/// 
/// Creates `.zbobr/{branch_name}` file, stages it, and commits it.
/// Does NOT push — the caller is responsible for pushing if needed.
/// 
/// Git user configuration should be set up by clone_and_setup or clone_readonly before calling this.
/// 
/// # Arguments
/// * `work_dir` - The repository working directory
/// * `branch_name` - The branch name (used for file naming and commit message)
pub async fn create_placeholder_commit(
    work_dir: &std::path::Path,
    branch_name: &str,
) -> Result<(), ZbobrError> {
    let zbobr_dir = work_dir.join(".zbobr");
    let placeholder_path = zbobr_dir.join(branch_name);

    // Create .zbobr directory
    tokio::fs::create_dir_all(&zbobr_dir)
        .await
        .map_err(|e| ZbobrError::Other(format!("Failed to create .zbobr directory: {}", e)))?;

    // Create placeholder file
    tokio::fs::File::create(&placeholder_path)
        .await
        .map_err(|e| ZbobrError::Other(format!("Failed to create placeholder file: {}", e)))?;

    // Stage the file
    let add_status = tokio::process::Command::new("git")
        .args(["add", &format!(".zbobr/{}", branch_name)])
        .current_dir(work_dir)
        .status()
        .await
        .map_err(|e| ZbobrError::Other(format!("Failed to run git add: {}", e)))?;

    if !add_status.success() {
        return Err(ZbobrError::Other("git add for placeholder failed".to_string()));
    }

    // Commit the file
    let commit_msg = format!("chore: add branch placeholder {}", branch_name);
    let commit_status = tokio::process::Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(work_dir)
        .status()
        .await
        .map_err(|e| ZbobrError::Other(format!("Failed to run git commit: {}", e)))?;

    if !commit_status.success() {
        return Err(ZbobrError::Other("git commit for placeholder failed".to_string()));
    }

    Ok(())
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
        parameters: std::collections::HashMap<Parameter, String>,
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

    /// Update the task description with optimistic locking to prevent concurrent update conflicts.
    /// This method attempts to write the new description while ensuring no concurrent modifications
    /// have occurred since the last read. If a conflict is detected, it automatically retries by
    /// re-reading the current state and reapplying the update.
    /// 
    /// The `expected_description` parameter should be the description value that was read from
    /// the task before modifications. If the current task description doesn't match this value,
    /// a concurrent modification is detected and the update is retried.
    async fn update_task_description_with_conflict_detection(
        &self,
        id: u64,
        expected_description: &str,
        new_description: &str,
    ) -> Result<(), ZbobrError>;

    /// List open tasks with a given stage name, optionally filtered by tool.
    async fn list_tasks_by_stage(
        &self,
        stage_name: &str,
        tool: Option<Tool>,
    ) -> Result<Vec<Task>, ZbobrError>;

    /// Check if a task is closed.
    async fn is_task_closed(&self, id: u64) -> Result<bool, ZbobrError>;

    /// Check if a file exists in the task repo.
    async fn repo_file_exists(&self, path: &str) -> Result<bool, ZbobrError>;

    /// Create or update a file in the task repo.
    async fn create_repo_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<(), ZbobrError>;

    /// Ensure the task repo exists.
    async fn ensure_task_repo_exists(&self) -> Result<(), ZbobrError>;

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

    /// Ensure the fork under `fork_owner` is synchronized with the upstream `target_repo` on `branch`.
    /// This performs a server-side sync (same as GitHub "Sync fork" button) and is
    /// not a local operation on the cloned copy.
    async fn sync_fork(&self, target_repo: &str, branch: &str) -> Result<(), ZbobrError>;

    /// Create a PR from work_branch to destination_branch in the fork repo.
    /// Returns the PR URL on success, or empty string on stub backend.
    async fn create_pr_in_fork(
        &self,
        destination_repository: &str,
        work_branch: &str,
        destination_branch: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError>;

    // -- Setup methods --

    /// List all stages (milestones) in the task repo.
    async fn list_stages(&self) -> Result<Vec<(u64, String)>, ZbobrError>;

    /// Create a stage (milestone) in the task repo.
    async fn create_stage(&self, title: &str, description: &str) -> Result<(), ZbobrError>;

    /// Delete a stage by its number.
    async fn delete_stage(&self, number: u64) -> Result<(), ZbobrError>;

    /// List all labels in the task repo.
    async fn list_labels(&self) -> Result<Vec<String>, ZbobrError>;

    /// Create a label in the task repo.
    async fn create_label(
        &self,
        name: &str,
        color: &str,
        description: &str,
    ) -> Result<(), ZbobrError>;

    /// Initialize the task repository with stages and labels.
    /// If force is true, overwrites existing labels.
    async fn setup_repository(&self, force: bool) -> Result<(), ZbobrError>;

    /// Validate that the backend can reach required resources (fork owner, task repo, etc.).
    async fn validate_connectivity(&self) -> Result<(), ZbobrError>;

    /// Return a debug string of the backend state.
    fn debug_state(&self) -> String;
}

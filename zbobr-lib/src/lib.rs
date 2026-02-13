pub mod backend;
pub mod cleanup;
pub mod config;
pub mod manager;
pub mod mcp;
pub mod setup;
pub mod task;
pub mod tool_executor;

use std::{path::PathBuf, sync::Arc};

pub use config::{TomlConfig, ZbobrConfig};
pub use mcp::{planner_instructions, worker_instructions, reviewer_instructions, PlannerMcp, WorkerMcp, ReviewerMcp};
pub use task::{Model, ChecklistItem, Parameter, Signal, Stage, Task, TaskSession, Tool};
pub use tool_executor::{ClaudeExecutor, CopilotExecutor, StubExecutor, ToolExecutor};

use crate::{
    backend::{github::GitHubBackend, stub::StubBackend, Backend},
    config::BackendType,
};

/// Central struct holding configuration and backend.
#[derive(Clone)]
pub struct Zbobr {
    config: Arc<ZbobrConfig>,
    backend: Arc<dyn Backend>,
}

impl Zbobr {
    /// Create a new Zbobr instance from config.
    pub fn new(config: ZbobrConfig) -> Result<Self, ZbobrError> {
        let config_arc = Arc::new(config.clone());
        let backend: Arc<dyn Backend> = match config.backend {
            BackendType::GitHub => {
                let octocrab = octocrab::Octocrab::builder()
                    .personal_token(config.owner_github_token.clone())
                    .build()
                    .map_err(|e| ZbobrError::GitHub(e.to_string()))?;
                Arc::new(GitHubBackend::new(config_arc.clone(), octocrab))
            }
            BackendType::Stub => Arc::new(StubBackend::new(config.workspace.clone())),
        };

        Ok(Self {
            config: config_arc,
            backend,
        })
    }

    pub fn config(&self) -> &ZbobrConfig {
        &self.config
    }

    /// Create a TaskSession bound to a specific task.
    pub fn task_session(&self, task_id: u64) -> TaskSession {
        TaskSession::new(self.clone(), task_id)
    }

    /// Validate that the backend can reach required resources (fork owner, task repo, etc.).
    pub async fn validate_connectivity(&self) -> Result<(), ZbobrError> {
        self.backend.validate_connectivity().await
    }

    // -- Delegate to Backend --

    pub async fn get_task(&self, id: u64) -> Result<Task, ZbobrError> {
        self.backend.get_task(id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_task(
        &self,
        title: &str,
        description: &str,
        stage: Stage,
        tool: Option<Tool>,
        model: Option<Model>,
        destination_repository: Option<String>,
        destination_branch: Option<String>,
    ) -> Result<u64, ZbobrError> {
        let mut parameters = std::collections::HashMap::new();
        if let Some(repo) = destination_repository {
            parameters.insert(Parameter::DestinationRepository, repo);
        }
        if let Some(branch) = destination_branch {
            parameters.insert(Parameter::DestinationBranch, branch);
        }
        
        self.backend.create_task(title, description, stage, tool, model, parameters).await
    }

    pub async fn close_task(&self, id: u64) -> Result<(), ZbobrError> {
        self.backend.close_task(id).await
    }

    pub async fn get_task_comments(&self, id: u64) -> Result<Vec<String>, ZbobrError> {
        self.backend.get_task_comments(id).await
    }

    pub async fn post_task_comment(
        &self,
        id: u64,
        body: &str,
        role: &str,
        hostname: &str,
    ) -> Result<(), ZbobrError> {
        self.backend
            .post_task_comment(id, body, role, hostname)
            .await
    }

    pub async fn set_task_stage_by_name(
        &self,
        id: u64,
        stage_name: &str,
    ) -> Result<(), ZbobrError> {
        self.backend.set_task_stage(id, stage_name).await
    }

    pub async fn set_task_signal(&self, id: u64, signal: Option<Signal>) -> Result<(), ZbobrError> {
        self.backend.set_task_signal(id, signal).await
    }

    pub async fn update_task_description(
        &self,
        id: u64,
        description: &str,
    ) -> Result<(), ZbobrError> {
        self.backend.update_task_description(id, description).await
    }

    /// Update task with plan and checklist (higher-level, handles serialization internally).
    /// Uses optimistic locking to prevent concurrent update conflicts.
    pub async fn update_task_plan(
        &self,
        id: u64,
        description: &str,
        plan: &str,
        checklist: &[ChecklistItem],
    ) -> Result<(), ZbobrError> {
        use crate::backend::serialize_description_with_plan_and_checklist;
        
        // Read the current task to get its current description (for conflict detection)
        let current_task = self.backend.get_task(id).await?;
        let expected_description = current_task.etag.as_deref().unwrap_or(&current_task.description);
        
        let full_description = serialize_description_with_plan_and_checklist(description, plan, checklist);
        self.backend
            .update_task_description_with_conflict_detection(id, expected_description, &full_description)
            .await
    }

    /// Update task with checklist (preserves existing plan if any).
    /// Uses optimistic locking to prevent concurrent update conflicts.
    pub async fn update_task_checklist(
        &self,
        id: u64,
        description: &str,
        checklist: &[ChecklistItem],
    ) -> Result<(), ZbobrError> {
        use crate::backend::serialize_description_with_checklist;
        
        // Read the current task to get its current description (for conflict detection)
        let current_task = self.backend.get_task(id).await?;
        let expected_description = current_task.etag.as_deref().unwrap_or(&current_task.description);
        
        let full_description = serialize_description_with_checklist(description, checklist);
        self.backend
            .update_task_description_with_conflict_detection(id, expected_description, &full_description)
            .await
    }

    /// Update task description with conflict detection.
    /// Internal helper used by higher-level update methods.
    pub(crate) async fn update_task_description_with_conflict_detection(
        &self,
        id: u64,
        expected_description: &str,
        new_description: &str,
    ) -> Result<(), ZbobrError> {
        self.backend
            .update_task_description_with_conflict_detection(id, expected_description, new_description)
            .await
    }

    /// Update task with description, parameters, plan, and checklist.
    /// Uses optimistic locking to prevent concurrent update conflicts.
    pub async fn update_task_full(
        &self,
        id: u64,
        description: &str,
        parameters: &std::collections::HashMap<Parameter, String>,
        plan: &str,
        checklist: &[ChecklistItem],
    ) -> Result<(), ZbobrError> {
        use crate::backend::serialize_description_full;
        
        // Read the current task to get its current description (for conflict detection)
        let current_task = self.backend.get_task(id).await?;
        let expected_description = current_task.etag.as_deref().unwrap_or(&current_task.description);
        
        // Convert Parameter enum keys to string keys for serialization
        let string_params: std::collections::HashMap<String, String> = parameters
            .iter()
            .map(|(k, v)| (k.name().to_string(), v.clone()))
            .collect();
        
        let full_description = serialize_description_full(description, &string_params, plan, checklist);
        self.backend
            .update_task_description_with_conflict_detection(id, expected_description, &full_description)
            .await
    }

    pub async fn list_tasks_by_stage(
        &self,
        stage_name: &str,
        tool: Option<Tool>,
    ) -> Result<Vec<Task>, ZbobrError> {
        self.backend.list_tasks_by_stage(stage_name, tool).await
    }

    pub async fn is_task_closed(&self, id: u64) -> Result<bool, ZbobrError> {
        self.backend.is_task_closed(id).await
    }

    pub async fn repo_file_exists(&self, path: &str) -> Result<bool, ZbobrError> {
        self.backend.repo_file_exists(path).await
    }

    pub async fn create_repo_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<(), ZbobrError> {
        self.backend
            .create_repo_file(path, content, commit_message)
            .await
    }

    pub async fn setup_repository(&self, force: bool) -> Result<(), ZbobrError> {
        self.backend.setup_repository(force).await
    }

    pub async fn ensure_task_repo_exists(&self) -> Result<(), ZbobrError> {
        self.backend.ensure_task_repo_exists().await
    }

    pub async fn clone_and_setup(
        &self,
        target_repo: &str,
        branch: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        self.backend
            .clone_and_setup(target_repo, branch, task_id)
            .await
    }

    pub async fn clone_readonly(
        &self,
        target_repo: &str,
        branch: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        self.backend
            .clone_readonly(target_repo, branch, task_id)
            .await
    }

    /// Parse PR reference to (repo, branch).
    /// Accepts formats:
    /// - "https://github.com/owner/repo/pull/123"
    /// - "owner/repo#123"
    ///   Returns (repo, branch_name)
    pub async fn parse_pr_to_repo_branch(
        &self,
        pr_ref: &str,
    ) -> Result<(String, String), ZbobrError> {
        self.backend.parse_pr_to_repo_branch(pr_ref).await
    }

    pub async fn push_and_create_pr(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError> {
        self.backend.push_and_create_pr(target_repo, task_id).await
    }

    pub async fn create_pr_in_fork(
        &self,
        destination_repository: &str,
        work_branch: &str,
        destination_branch: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError> {
        self.backend
            .create_pr_in_fork(destination_repository, work_branch, destination_branch, task_id)
            .await
    }

    pub async fn list_stages(&self) -> Result<Vec<(u64, String)>, ZbobrError> {
        self.backend.list_stages().await
    }

    pub async fn create_stage(&self, title: &str, description: &str) -> Result<(), ZbobrError> {
        self.backend.create_stage(title, description).await
    }

    pub async fn delete_stage(&self, number: u64) -> Result<(), ZbobrError> {
        self.backend.delete_stage(number).await
    }

    pub async fn list_labels(&self) -> Result<Vec<String>, ZbobrError> {
        self.backend.list_labels().await
    }

    pub async fn create_label(
        &self,
        name: &str,
        color: &str,
        description: &str,
    ) -> Result<(), ZbobrError> {
        self.backend.create_label(name, color, description).await
    }

    pub fn debug_state(&self) -> String {
        self.backend.debug_state()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ZbobrError {
    #[error("GitHub API error: {0}")]
    GitHub(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl From<octocrab::Error> for ZbobrError {
    fn from(e: octocrab::Error) -> Self {
        // Provide more detailed error information
        let error_msg = match e {
            octocrab::Error::GitHub { source, .. } => {
                format!(
                    "GitHub API error: {} (status: {})",
                    source.message, source.status_code
                )
            }
            _ => format!("GitHub API error: {}", e),
        };
        ZbobrError::GitHub(error_msg)
    }
}

pub mod backend;
pub mod cleanup;
pub mod config;
pub mod manager;
pub mod mcp;
pub mod setup;
pub mod task;
pub mod tool_executor;

pub use config::ZbobrConfig;
pub use task::{Model, PlannerSession, SetupFile, Stage, Task, Tool, WorkerSession};
pub use tool_executor::{ClaudeExecutor, CopilotExecutor, StubExecutor, ToolExecutor};

use crate::backend::github::GitHubBackend;
use crate::backend::stub::StubBackend;
use crate::backend::Backend;
use crate::config::BackendType;
use std::path::PathBuf;
use std::sync::Arc;

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
                    .personal_token(config.github_token.clone())
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

    /// Create a PlannerSession bound to a specific task.
    pub fn planner_session(&self, task_id: u64) -> PlannerSession {
        PlannerSession::new(self.clone(), task_id)
    }

    /// Create a WorkerSession bound to a specific task.
    pub fn worker_session(&self, task_id: u64) -> WorkerSession {
        WorkerSession::new(self.clone(), task_id)
    }

    // -- Delegate to Backend --

    pub async fn get_task(&self, id: u64) -> Result<Task, ZbobrError> {
        self.backend.get_task(id).await
    }

    pub async fn create_task(
        &self,
        title: &str,
        description: &str,
        stage: Stage,
        tool: Option<Tool>,
        model: Option<Model>,
        parent_task_id: Option<u64>,
        destination_repo: Option<String>,
        destination_branch: Option<String>,
    ) -> Result<u64, ZbobrError> {
        self.backend
            .create_task(
                title,
                description,
                stage,
                tool,
                model,
                parent_task_id,
                destination_repo,
                destination_branch,
            )
            .await
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
        self.backend.post_task_comment(id, body, role, hostname).await
    }

    pub async fn set_task_stage_by_name(
        &self,
        id: u64,
        stage_name: &str,
    ) -> Result<(), ZbobrError> {
        self.backend.set_task_stage(id, stage_name).await
    }

    pub async fn add_task_label(&self, id: u64, label: &str) -> Result<(), ZbobrError> {
        self.backend.add_task_label(id, label).await
    }

    pub async fn remove_task_label(&self, id: u64, label: &str) -> Result<(), ZbobrError> {
        self.backend.remove_task_label(id, label).await
    }

    pub async fn update_task_description(
        &self,
        id: u64,
        description: &str,
    ) -> Result<(), ZbobrError> {
        self.backend.update_task_description(id, description).await
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

    pub async fn setup_repository(&self, files: &[SetupFile], force: bool) -> Result<(), ZbobrError> {
        self.backend.setup_repository(files, force).await
    }

    pub async fn ensure_domain_repo_exists(&self) -> Result<(), ZbobrError> {
        self.backend.ensure_domain_repo_exists().await
    }

    pub async fn clone_and_setup(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        self.backend.clone_and_setup(target_repo, task_id).await
    }

    pub async fn clone_readonly(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        self.backend.clone_readonly(target_repo, task_id).await
    }

    pub async fn push_and_create_pr(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError> {
        self.backend.push_and_create_pr(target_repo, task_id).await
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
        ZbobrError::GitHub(e.to_string())
    }
}

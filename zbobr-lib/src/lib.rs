pub mod config;
mod github;
pub mod setup;
pub mod manager;
pub mod cleanup;
pub mod task;

pub use config::ZbobrConfig;
pub use task::{Stage, Task, PlannerSession, WorkerSession};

use std::sync::Arc;

/// Central struct holding configuration and GitHub client.
#[derive(Clone)]
pub struct Zbobr {
    config: Arc<ZbobrConfig>,
    octocrab: octocrab::Octocrab,
}

impl Zbobr {
    /// Create a new Zbobr instance from config.
    pub fn new(config: ZbobrConfig) -> Result<Self, ZbobrError> {
        let octocrab = octocrab::Octocrab::builder()
            .personal_token(config.github_token.clone())
            .build()
            .map_err(|e| ZbobrError::GitHub(e.to_string()))?;
        Ok(Self {
            config: Arc::new(config),
            octocrab,
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

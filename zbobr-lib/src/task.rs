use crate::{Zbobr, ZbobrError};

/// Workflow stage (maps to GitHub milestones internally).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum Stage {
    Pending,
    PlanningReady,
    Planning,
    WorkingReady,
    Working,
}

impl Stage {
    pub fn milestone_name(&self) -> &'static str {
        match self {
            Stage::Pending => "PENDING",
            Stage::PlanningReady => "PLANNING_READY",
            Stage::Planning => "PLANNING",
            Stage::WorkingReady => "WORKING_READY",
            Stage::Working => "WORKING",
        }
    }

    pub fn from_milestone_name(name: &str) -> Option<Self> {
        match name {
            "PENDING" => Some(Stage::Pending),
            "PLANNING_READY" => Some(Stage::PlanningReady),
            "PLANNING" => Some(Stage::Planning),
            "WORKING_READY" => Some(Stage::WorkingReady),
            "WORKING" => Some(Stage::Working),
            _ => None,
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.milestone_name())
    }
}

/// AI Tool/Agent to use.
#[derive(
    Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
pub enum Tool {
    #[serde(rename = "copilot")]
    Copilot,
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "stub")]
    Stub,
}

impl std::fmt::Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tool::Copilot => write!(f, "copilot"),
            Tool::Claude => write!(f, "claude"),
            Tool::Stub => write!(f, "stub"),
        }
    }
}

impl std::str::FromStr for Tool {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "copilot" => Ok(Tool::Copilot),
            "claude" => Ok(Tool::Claude),
            "stub" => Ok(Tool::Stub),
            _ => Err(format!("Unknown tool: {}", s)),
        }
    }
}

/// AI Model to use.
#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
pub enum Model {
    #[serde(rename = "gpt-4o")]
    Gpt4o,
    #[serde(rename = "gpt-5-mini")]
    Gpt5Mini,
    #[serde(rename = "claude-3-5-sonnet")]
    Claude35Sonnet,
    #[serde(rename = "claude-3-opus")]
    Claude3Opus,
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Model::Gpt4o => write!(f, "gpt-4o"),
            Model::Gpt5Mini => write!(f, "gpt-5-mini"),
            Model::Claude35Sonnet => write!(f, "claude-3-5-sonnet"),
            Model::Claude3Opus => write!(f, "claude-3-opus"),
        }
    }
}

impl std::str::FromStr for Model {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('.', "-").as_str() {
            "gpt-4o" | "gpt4o" => Ok(Model::Gpt4o),
            "gpt-5-mini" | "gpt5mini" | "gpt-5" => Ok(Model::Gpt5Mini),
            "claude-3-5-sonnet" | "claude35sonnet" | "claude-3.5-sonnet" => {
                Ok(Model::Claude35Sonnet)
            }
            "claude-3-opus" | "claude3opus" => Ok(Model::Claude3Opus),
            _ => Err(format!("Unknown model: {}", s)),
        }
    }
}

/// A task in the abstract domain (generic, backed by GitHub or Stub).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub discussion: Vec<String>,
    pub stage: Stage,
    pub tool: Option<Tool>,
    pub model: Option<Model>,
    pub parent_task_id: Option<u64>,
    pub destination_repo: Option<String>,
    pub destination_branch: Option<String>,
    pub done: bool,
}

/// Planner session bound to a specific task.
pub struct PlannerSession {
    zbobr: Zbobr,
    task_id: u64,
}

impl PlannerSession {
    pub(crate) fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self { zbobr, task_id }
    }

    pub fn task_id(&self) -> u64 {
        self.task_id
    }

    /// Get the current task description.
    pub async fn get_description(&self) -> Result<String, ZbobrError> {
        let task = self.zbobr.get_task(self.task_id).await?;
        Ok(task.description)
    }

    /// Update the task description.
    pub async fn set_description(&self, description: &str) -> Result<(), ZbobrError> {
        self.zbobr
            .update_task_description(self.task_id, description)
            .await
    }

    /// Get all discussion messages on the task.
    pub async fn get_discussion(&self) -> Result<Vec<String>, ZbobrError> {
        self.zbobr.get_task_comments(self.task_id).await
    }

    /// Post a message to the task discussion.
    pub async fn post_message(&self, msg: &str) -> Result<(), ZbobrError> {
        self.zbobr.post_task_comment(self.task_id, msg).await
    }

    /// Clone target repo locally for investigation (read-only).
    pub async fn request_repo(&self, repo: &str) -> Result<String, ZbobrError> {
        let path = self.zbobr.clone_readonly(repo, self.task_id).await?;
        Ok(path.to_string_lossy().to_string())
    }
}

/// Worker session bound to a specific task.
pub struct WorkerSession {
    zbobr: Zbobr,
    task_id: u64,
}

impl WorkerSession {
    pub(crate) fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self { zbobr, task_id }
    }

    pub fn task_id(&self) -> u64 {
        self.task_id
    }

    /// Get the current task description.
    pub async fn get_description(&self) -> Result<String, ZbobrError> {
        let task = self.zbobr.get_task(self.task_id).await?;
        Ok(task.description)
    }

    /// Get all discussion messages.
    pub async fn get_discussion(&self) -> Result<Vec<String>, ZbobrError> {
        self.zbobr.get_task_comments(self.task_id).await
    }

    /// Post a message to the task discussion.
    pub async fn post_message(&self, msg: &str) -> Result<(), ZbobrError> {
        self.zbobr.post_task_comment(self.task_id, msg).await
    }

    /// Fork target repo, clone locally, create branch, return local path.
    pub async fn request_repo(&self, repo: &str) -> Result<String, ZbobrError> {
        let path = self.zbobr.clone_and_setup(repo, self.task_id).await?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Push changes and create PR from the prepared branch.
    pub async fn submit_work(&self, target_repo: &str) -> Result<String, ZbobrError> {
        self.zbobr
            .push_and_create_pr(target_repo, self.task_id)
            .await
    }

    /// Mark task as done (sets done flag and transitions to Pending).
    pub async fn mark_done(&self) -> Result<(), ZbobrError> {
        self.zbobr
            .set_task_stage(self.task_id, Stage::Pending)
            .await?;
        self.zbobr.add_task_label(self.task_id, "done").await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_milestone_names() {
        assert_eq!(Stage::Pending.milestone_name(), "PENDING");
        assert_eq!(Stage::PlanningReady.milestone_name(), "PLANNING_READY");
        assert_eq!(Stage::Planning.milestone_name(), "PLANNING");
        assert_eq!(Stage::WorkingReady.milestone_name(), "WORKING_READY");
        assert_eq!(Stage::Working.milestone_name(), "WORKING");
    }

    #[test]
    fn stage_display() {
        assert_eq!(Stage::Planning.to_string(), "PLANNING");
        assert_eq!(Stage::Working.to_string(), "WORKING");
    }

    #[test]
    fn stage_roundtrip_serde() {
        let stage = Stage::PlanningReady;
        let json = serde_json::to_string(&stage).unwrap();
        let back: Stage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, stage);
    }

    #[test]
    fn task_serde() {
        let task = Task {
            id: 42,
            title: "Test task".to_string(),
            description: "Do something".to_string(),
            discussion: vec!["Hello".to_string()],
            stage: Stage::Planning,
            tool: Some(Tool::Claude),
            model: Some(Model::Claude3Opus),
            parent_task_id: None,
            destination_repo: None,
            destination_branch: None,
            done: false,
        };
        let json = serde_json::to_string(&task).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 42);
        assert_eq!(back.title, "Test task");
        assert_eq!(back.stage, Stage::Planning);
        assert_eq!(back.tool, Some(Tool::Claude));
        assert_eq!(back.model, Some(Model::Claude3Opus));
        assert!(!back.done);
    }
}

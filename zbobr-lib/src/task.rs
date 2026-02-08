use crate::{Zbobr, ZbobrError};

/// A file to create in the domain repository during setup.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SetupFile {
    /// Path relative to the repo root (e.g., "README.md").
    pub path: String,
    /// File content (plain text).
    pub content: String,
}

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

impl Tool {
    /// Returns all available tools.
    pub fn all() -> &'static [Tool] {
        &[Tool::Copilot, Tool::Claude, Tool::Stub]
    }

    /// Returns the appropriate executor for this tool.
    pub fn executor(&self) -> Box<dyn crate::tool_executor::ToolExecutor> {
        match self {
            Tool::Copilot => Box::new(crate::tool_executor::CopilotExecutor),
            Tool::Claude => Box::new(crate::tool_executor::ClaudeExecutor),
            Tool::Stub => Box::new(crate::tool_executor::StubExecutor),
        }
    }
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
    #[serde(rename = "claude-sonnet-4.5")]
    ClaudeSonnet4_5,
    #[serde(rename = "claude-haiku-4.5")]
    ClaudeHaiku4_5,
    #[serde(rename = "claude-opus-4.6")]
    ClaudeOpus4_6,
    #[serde(rename = "claude-opus-4.5")]
    ClaudeOpus4_5,
    #[serde(rename = "claude-sonnet-4")]
    ClaudeSonnet4,
    #[serde(rename = "gemini-3-pro-preview")]
    Gemini3ProPreview,
    #[serde(rename = "gpt-5.2-codex")]
    Gpt5_2Codex,
    #[serde(rename = "gpt-5.2")]
    Gpt5_2,
    #[serde(rename = "gpt-5.1-codex-max")]
    Gpt5_1CodexMax,
    #[serde(rename = "gpt-5.1-codex")]
    Gpt5_1Codex,
}

impl Model {
    /// Returns all available models.
    pub fn all() -> &'static [Model] {
        &[
            Model::Gpt4o,
            Model::Gpt5Mini,
            Model::Claude35Sonnet,
            Model::Claude3Opus,
            Model::ClaudeSonnet4_5,
            Model::ClaudeHaiku4_5,
            Model::ClaudeOpus4_6,
            Model::ClaudeOpus4_5,
            Model::ClaudeSonnet4,
            Model::Gemini3ProPreview,
            Model::Gpt5_2Codex,
            Model::Gpt5_2,
            Model::Gpt5_1CodexMax,
            Model::Gpt5_1Codex,
        ]
    }

    pub fn model_name_for_tool(&self, tool: Tool) -> Option<&'static str> {
        match tool {
            Tool::Copilot => match self {
                Model::Gpt4o => Some("gpt-4o"),
                Model::Gpt5Mini => Some("gpt-5-mini"),
                Model::Claude35Sonnet => Some("claude-3-5-sonnet"),
                Model::Claude3Opus => Some("claude-3-opus"),
                Model::ClaudeSonnet4_5 => Some("claude-sonnet-4.5"),
                Model::ClaudeHaiku4_5 => Some("claude-haiku-4.5"),
                Model::ClaudeOpus4_6 => Some("claude-opus-4.6"),
                Model::ClaudeOpus4_5 => Some("claude-opus-4.5"),
                Model::ClaudeSonnet4 => Some("claude-sonnet-4"),
                Model::Gemini3ProPreview => Some("gemini-3-pro-preview"),
                Model::Gpt5_2Codex => Some("gpt-5.2-codex"),
                Model::Gpt5_2 => Some("gpt-5.2"),
                Model::Gpt5_1CodexMax => Some("gpt-5.1-codex-max"),
                Model::Gpt5_1Codex => Some("gpt-5.1-codex"),
            },
            Tool::Claude => match self {
                Model::Claude35Sonnet => Some("sonnet"),
                Model::Claude3Opus => Some("opus"),
                _ => None,
            },
            Tool::Stub => Some("stub-model"),
        }
    }
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Model::Gpt4o => "gpt-4o",
            Model::Gpt5Mini => "gpt-5-mini",
            Model::Claude35Sonnet => "claude-3-5-sonnet",
            Model::Claude3Opus => "claude-3-opus",
            Model::ClaudeSonnet4_5 => "claude-sonnet-4.5",
            Model::ClaudeHaiku4_5 => "claude-haiku-4.5",
            Model::ClaudeOpus4_6 => "claude-opus-4.6",
            Model::ClaudeOpus4_5 => "claude-opus-4.5",
            Model::ClaudeSonnet4 => "claude-sonnet-4",
            Model::Gemini3ProPreview => "gemini-3-pro-preview",
            Model::Gpt5_2Codex => "gpt-5.2-codex",
            Model::Gpt5_2 => "gpt-5.2",
            Model::Gpt5_1CodexMax => "gpt-5.1-codex-max",
            Model::Gpt5_1Codex => "gpt-5.1-codex",
        };
        f.write_str(s)
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
            "claude-sonnet-4.5" | "claude-sonnet-4-5" => Ok(Model::ClaudeSonnet4_5),
            "claude-haiku-4.5" | "claude-haiku-4-5" => Ok(Model::ClaudeHaiku4_5),
            "claude-opus-4.6" | "claude-opus-4-6" => Ok(Model::ClaudeOpus4_6),
            "claude-opus-4.5" | "claude-opus-4-5" => Ok(Model::ClaudeOpus4_5),
            "claude-sonnet-4" => Ok(Model::ClaudeSonnet4),
            "gemini-3-pro-preview" => Ok(Model::Gemini3ProPreview),
            "gpt-5.2-codex" | "gpt-5-2-codex" => Ok(Model::Gpt5_2Codex),
            "gpt-5.2" | "gpt-5-2" => Ok(Model::Gpt5_2),
            "gpt-5.1-codex-max" | "gpt-5-1-codex-max" => Ok(Model::Gpt5_1CodexMax),
            "gpt-5.1-codex" | "gpt-5-1-codex" => Ok(Model::Gpt5_1Codex),
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

    #[test]
    fn model_mapping() {
        assert_eq!(
            Model::Gpt4o.model_name_for_tool(Tool::Copilot),
            Some("gpt-4o")
        );
        assert_eq!(
            Model::ClaudeSonnet4_5.model_name_for_tool(Tool::Copilot),
            Some("claude-sonnet-4.5")
        );
        assert_eq!(
            Model::Claude35Sonnet.model_name_for_tool(Tool::Claude),
            Some("sonnet")
        );
        assert_eq!(Model::Gpt5_2.model_name_for_tool(Tool::Claude), None);
        assert_eq!(
            Model::Gpt5_1Codex.model_name_for_tool(Tool::Stub),
            Some("stub-model")
        );
    }

    #[test]
    fn model_parsing() {
        assert_eq!("gpt-5.2".parse::<Model>().unwrap(), Model::Gpt5_2);
        assert_eq!("GPT-5.2".parse::<Model>().unwrap(), Model::Gpt5_2);
        assert_eq!("gpt-5-2".parse::<Model>().unwrap(), Model::Gpt5_2);
        assert_eq!(
            "claude-sonnet-4.5".parse::<Model>().unwrap(),
            Model::ClaudeSonnet4_5
        );
        assert!("invalid-model".parse::<Model>().is_err());
    }
}

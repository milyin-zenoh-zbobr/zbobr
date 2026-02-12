use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{Zbobr, ZbobrError};

// -- Parameter names enum --

/// Standardized parameter names for task configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Parameter {
    DestinationRepository,
    DestinationBranch,
    WorkBranch,
    PrUrl,
}

impl Parameter {
    /// Returns the parameter name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            Parameter::DestinationRepository => "destination_repository",
            Parameter::DestinationBranch => "destination_branch",
            Parameter::WorkBranch => "work_branch",
            Parameter::PrUrl => "pr_url",
        }
    }
}

// -- Checklist item types --

/// A single item in a task's checklist.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct ChecklistItem {
    #[schemars(description = "Unique identifier for the checklist item")]
    pub id: String,
    #[schemars(description = "Checkbox state (true = checked, false = unchecked)")]
    pub checked: bool,
    #[schemars(description = "Checklist item text")]
    pub text: String,
}

/// Workflow stage (maps to GitHub milestones internally).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum Stage {
    Pending,
    GoPlanning,
    Planning,
    GoWorking,
    Working,
}

impl Stage {
    pub fn milestone_name(&self) -> &'static str {
        match self {
            Stage::Pending => "PENDING",
            Stage::GoPlanning => "GO_PLANNING",
            Stage::Planning => "PLANNING",
            Stage::GoWorking => "GO_WORKING",
            Stage::Working => "WORKING",
        }
    }

    pub fn from_milestone_name(name: &str) -> Option<Self> {
        match name {
            "PENDING" => Some(Stage::Pending),
            "GO_PLANNING" => Some(Stage::GoPlanning),
            "PLANNING" => Some(Stage::Planning),
            "GO_WORKING" => Some(Stage::GoWorking),
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

/// Role for task execution (planner or worker).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum Role {
    #[serde(rename = "planner")]
    Planner,
    #[serde(rename = "worker")]
    Worker,
}

impl Role {
    /// Returns the role name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Planner => "planner",
            Role::Worker => "worker",
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Role {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "planner" => Ok(Role::Planner),
            "worker" => Ok(Role::Worker),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

/// Signal for task flow control (mapped to labels in GitHub backend).
/// Ordered by priority (highest to lowest).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum Signal {
    #[serde(rename = "stop")]
    Stop = 0,
    #[serde(rename = "go_ask")]
    GoAsk = 1, // go ask is higher priority than done because it indicates a need for human input, 
               // while done supposes normal completion and less urgency. So go_ask should override done
               // if both are signalled by an agent.
    #[serde(rename = "done")]
    Done = 2,
    #[serde(rename = "go_work")]
    GoWork = 3,
    #[serde(rename = "go_plan")]
    GoPlan = 4,
}

impl Signal {
    /// Returns the plain signal name.
    pub fn name(&self) -> &'static str {
        match self {
            Signal::Stop => "stop",
            Signal::Done => "done",
            Signal::GoAsk => "go_ask",
            Signal::GoWork => "go_work",
            Signal::GoPlan => "go_plan",
        }
    }

    /// Returns all available signals in priority order.
    pub fn all() -> &'static [Signal] {
        &[Signal::Stop, Signal::Done, Signal::GoAsk, Signal::GoWork, Signal::GoPlan]
    }

    /// Maps signal to target stage.
    pub fn target_stage(&self) -> Stage {
        match self {
            Signal::Stop | Signal::Done | Signal::GoAsk => Stage::Pending,
            Signal::GoWork => Stage::GoWorking,
            Signal::GoPlan => Stage::GoPlanning,
        }
    }
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for Signal {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('_', "").as_str() {
            "stop" => Ok(Signal::Stop),
            "done" => Ok(Signal::Done),
            "goask" | "go-ask" => Ok(Signal::GoAsk),
            "gowork" | "go-work" => Ok(Signal::GoWork),
            "goplan" | "go-plan" => Ok(Signal::GoPlan),
            _ => Err(format!("Unknown signal: {}", s)),
        }
    }
}

/// AI Tool/Agent to use.
#[derive(
    Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[derive(Default)]
pub enum Tool {
    #[serde(rename = "copilot")]
    #[default]
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
#[derive(Default)]
pub enum Model {
    #[serde(rename = "gpt-4o")]
    Gpt4o,
    #[serde(rename = "gpt-5-mini")]
    #[default]
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
    pub plan: String,
    pub discussion: Vec<String>,
    pub stage: Stage,
    pub tool: Option<Tool>,
    pub model: Option<Model>,
    pub parent_task_id: Option<u64>,
    pub parameters: HashMap<Parameter, String>,
    pub done: bool,
    pub checklist: Vec<ChecklistItem>,
    pub signal: Option<Signal>,
}

/// Tracked repository information for a task session.
#[derive(Debug, Clone)]
struct TrackedRepo {
    repo: String,
    local_path: std::path::PathBuf,
}

/// Task session bound to a specific task, with role-based behavior.
#[derive(Clone)]
pub struct TaskSession {
    zbobr: Zbobr,
    task_id: u64,
    tracked_repos: Arc<Mutex<HashMap<String, TrackedRepo>>>,
}

impl TaskSession {
    pub(crate) fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self {
            zbobr,
            task_id,
            tracked_repos: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn task_id(&self) -> u64 {
        self.task_id
    }

    /// Create a branch name with the proper prefix for this task.
    pub fn create_branch_name(&self, short_name: &str) -> String {
        format!(
            "{}/{}/{}",
            self.zbobr.config().work_branch_prefix,
            self.task_id,
            short_name
        )
    }

    /// Check whether a branch name starts with this task's prefix.
    pub fn validate_branch_prefix(&self, branch: &str) -> bool {
        let prefix = format!(
            "{}/{}/",
            self.zbobr.config().work_branch_prefix,
            self.task_id
        );
        branch.starts_with(&prefix)
    }

    /// Get the current task description.
    pub async fn get_description(&self) -> Result<String, ZbobrError> {
        let task = self.zbobr.get_task(self.task_id).await?;
        Ok(task.description)
    }

    /// Get the current task plan.
    pub async fn get_plan(&self) -> Result<String, ZbobrError> {
        let task = self.zbobr.get_task(self.task_id).await?;
        Ok(task.plan)
    }

    /// Get the current task checklist.
    pub async fn get_checklist(&self) -> Result<Vec<ChecklistItem>, ZbobrError> {
        let task = self.zbobr.get_task(self.task_id).await?;
        Ok(task.checklist)
    }

    /// Update the task plan.
    pub async fn update_plan(&self, description: &str, plan: &str, checklist: &[ChecklistItem]) -> Result<(), ZbobrError> {
        self.zbobr
            .update_task_plan(self.task_id, description, plan, checklist)
            .await
    }

    /// Update the task description and checklist separately.
    /// The checklist will be serialized into the description for storage via the backend.
    pub async fn update_checklist(&self, description: &str, checklist: &[ChecklistItem]) -> Result<(), ZbobrError> {
        self.zbobr
            .update_task_checklist(self.task_id, description, checklist)
            .await
    }

    /// Update the task checklist while preserving an explicit plan.
    pub async fn update_checklist_with_plan(
        &self,
        description: &str,
        plan: &str,
        checklist: &[ChecklistItem],
    ) -> Result<(), ZbobrError> {
        self.zbobr
            .update_task_plan(self.task_id, description, plan, checklist)
            .await
    }

    /// Update the task description.
    pub async fn update_description(&self, description: &str) -> Result<(), ZbobrError> {
        self.zbobr
            .update_task_description(self.task_id, description)
            .await
    }

    /// Get all discussion messages on the task.
    pub async fn get_discussion(&self) -> Result<Vec<String>, ZbobrError> {
        self.zbobr.get_task_comments(self.task_id).await
    }

    /// Post a message to the task discussion with role and hostname metadata.
    pub async fn post_message(
        &self,
        msg: &str,
        role: &str,
        hostname: &str,
    ) -> Result<(), ZbobrError> {
        self.zbobr
            .post_task_comment(self.task_id, msg, role, hostname)
            .await
    }

    /// Get the current signal on the task.
    pub async fn get_signal(&self) -> Result<Option<Signal>, ZbobrError> {
        let task = self.zbobr.get_task(self.task_id).await?;
        Ok(task.signal)
    }

    /// Set signal on the task, respecting priority (higher priority signals cannot be overwritten by lower).
    pub async fn set_signal(&self, new_signal: Signal) -> Result<(), ZbobrError> {
        let current = self.get_signal().await?;
        
        // Only set if new signal has higher or equal priority (lower enum value)
        if let Some(current_signal) = current
            && new_signal > current_signal
        {
            // new_signal has lower priority, don't overwrite
            return Ok(());
        }
        
        self.zbobr.set_task_signal(self.task_id, Some(new_signal)).await
    }

    /// Clear the signal on the task.
    pub async fn clear_signal(&self) -> Result<(), ZbobrError> {
        self.zbobr.set_task_signal(self.task_id, None).await
    }

    /// Transition task to stage based on current signal.
    pub async fn transition_by_signal(&self) -> Result<(), ZbobrError> {
        let signal = self.get_signal().await?;
        if let Some(sig) = signal {
            let target_stage = sig.target_stage();
            self.zbobr.set_task_stage(self.task_id, target_stage).await?;
        }
        Ok(())
    }

    /// Clone target repo and checkout specific branch (read-only, for planner).
    pub async fn request_branch_readonly(
        &self,
        repo: &str,
        branch: &str,
    ) -> Result<String, ZbobrError> {
        let path = self
            .zbobr
            .clone_readonly(repo, branch, self.task_id)
            .await?;
        let path_str = path.to_string_lossy().to_string();

        // Track this repo and branch
        let mut tracked = self.tracked_repos.lock().unwrap();
        tracked.insert(
            repo.to_string(),
            TrackedRepo {
                repo: repo.to_string(),
                local_path: path,
            },
        );

        Ok(path_str)
    }

    /// Fork target repo, clone locally, checkout specific branch (for worker).
    pub async fn request_branch(&self, repo: &str, branch: &str) -> Result<String, ZbobrError> {
        let path = self
            .zbobr
            .clone_and_setup(repo, branch, self.task_id)
            .await?;
        let path_str = path.to_string_lossy().to_string();

        // Track this repo and branch for later submit_work
        let mut tracked = self.tracked_repos.lock().unwrap();
        tracked.insert(
            repo.to_string(),
            TrackedRepo {
                repo: repo.to_string(),
                local_path: path,
            },
        );

        Ok(path_str)
    }

    /// Helper: Clone repo and checkout branch from PR.
    /// PR format: "https://github.com/owner/repo/pull/123" or "owner/repo#123"
    pub async fn request_branch_by_pr(
        &self,
        pr: &str,
        readonly: bool,
    ) -> Result<String, ZbobrError> {
        let (repo, branch) = self.zbobr.parse_pr_to_repo_branch(pr).await?;
        if readonly {
            self.request_branch_readonly(&repo, &branch).await
        } else {
            self.request_branch(&repo, &branch).await
        }
    }

    /// Push the current branch to the fork remote.
    /// Validates that the current branch has the correct task prefix.
    pub async fn push_branch(&self, path: &str) -> Result<(), ZbobrError> {
        let work_dir = std::path::PathBuf::from(path);

        if !work_dir.exists() {
            return Err(ZbobrError::Other(format!(
                "Work directory does not exist: {}",
                work_dir.display()
            )));
        }

        // Get current branch name
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&work_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(ZbobrError::Other("Failed to get current branch".into()));
        }

        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if !self.validate_branch_prefix(&current_branch) {
            return Err(ZbobrError::Other(format!(
                "Branch '{}' does not match expected prefix '{}/{}/'. Use create_branch_name to generate a valid branch name.",
                current_branch,
                self.zbobr.config().work_branch_prefix,
                self.task_id
            )));
        }

        // Push to fork
        tracing::info!("Pushing branch '{}' to fork", current_branch);
        let status = tokio::process::Command::new("git")
            .args(["push", "-u", "fork", "HEAD", "--force"])
            .current_dir(&work_dir)
            .status()
            .await?;

        if !status.success() {
            return Err(ZbobrError::Other("Failed to push to fork".into()));
        }

        Ok(())
    }

    /// Push the current branch and create a PR within the fork.
    /// The PR is created in the fork repo with `destination_branch` as base.
    pub async fn push_branch_and_create_pr(
        &self,
        path: &str,
        destination_branch: &str,
    ) -> Result<String, ZbobrError> {
        // First push the branch
        self.push_branch(path).await?;

        let work_dir = std::path::PathBuf::from(path);

        // Get current branch name (already validated by push_branch)
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&work_dir)
            .output()
            .await?;
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let fork_owner = &self.zbobr.config().fork_owner;

        // Find the repo name from tracked repos
        let repo_name = {
            let tracked = self.tracked_repos.lock().unwrap();
            tracked
                .values()
                .find(|r| r.local_path == work_dir)
                .map(|r| r.repo.split('/').nth(1).unwrap_or(&r.repo).to_string())
                .ok_or_else(|| {
                    ZbobrError::Other(format!(
                        "Path {} was not obtained from request_branch or request_branch_by_pr",
                        path
                    ))
                })?
        };

        let fork_repo = format!("{fork_owner}/{repo_name}");

        // Create PR within the fork
        let task = self.zbobr.get_task(self.task_id).await?;
        let pr_title = format!("Fix #{}: {}", self.task_id, task.title);
        let pr_body = format!(
            "Resolves #{}\n\nImplementation for: {}",
            self.task_id, task.title
        );

        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "create",
                "--repo",
                &fork_repo,
                "--head",
                &current_branch,
                "--base",
                destination_branch,
                "--title",
                &pr_title,
                "--body",
                &pr_body,
            ])
            .current_dir(&work_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ZbobrError::Other(format!("Failed to create PR: {stderr}")));
        }

        let pr_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(pr_url)
    }

    /// Push the work_branch in the cloned repository. Stashes local changes if a different branch is selected.
    /// The work repository has all remote information cleared - only pull_work and push_work know where to push.
    pub async fn push_work(&self) -> Result<(), ZbobrError> {
        // Get the destination repo (needed to find the cloned path)
        let dest_repo = self.get_parameter(Parameter::DestinationRepository.name()).await?
            .ok_or_else(|| ZbobrError::Other("destination_repository parameter not set".to_string()))?;
        
        // Find the work directory for this repository
        let work_dir = {
            let tracked = self.tracked_repos.lock().unwrap();
            tracked
                .get(&dest_repo)
                .map(|r| r.local_path.clone())
                .ok_or_else(|| ZbobrError::Other(format!(
                    "Repository {} has not been pulled with pull_work",
                    dest_repo
                )))?
        };

        if !work_dir.exists() {
            return Err(ZbobrError::Other(format!(
                "Work directory does not exist: {}",
                work_dir.display()
            )));
        }

        // Get the work_branch name
        let work_branch = self.get_parameter(Parameter::WorkBranch.name()).await?
            .ok_or_else(|| ZbobrError::Other("work_branch parameter not set".to_string()))?;

        // Get current branch
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&work_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(ZbobrError::Other("Failed to get current branch".into()));
        }

        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // If on a different branch, stash changes
        if current_branch != work_branch {
            tracing::info!("Stashing changes on branch '{}' before switching to '{}'", current_branch, work_branch);
            let stash_status = tokio::process::Command::new("git")
                .args(["stash"])
                .current_dir(&work_dir)
                .status()
                .await?;

            if !stash_status.success() {
                tracing::warn!("Stash may have failed or nothing to stash");
            }

            // Switch to work branch
            tracing::info!("Switching to branch '{}'", work_branch);
            let checkout_status = tokio::process::Command::new("git")
                .args(["checkout", &work_branch])
                .current_dir(&work_dir)
                .status()
                .await?;

            if !checkout_status.success() {
                return Err(ZbobrError::Other(format!(
                    "Failed to checkout branch '{}'",
                    work_branch
                )));
            }
        }

        // Push to the configured remote (set by pull_work)
        tracing::info!("Pushing branch '{}' to remote", work_branch);
        let status = tokio::process::Command::new("git")
            .args(["push", "-u", "origin", "HEAD", "--force"])
            .current_dir(&work_dir)
            .status()
            .await?;

        if !status.success() {
            return Err(ZbobrError::Other("Failed to push work branch".into()));
        }

        Ok(())
    }

    /// Pull a repository, forking if needed. Clones the destination_repository fork, creates and checks out work_branch.
    /// Cleans up remote information - only pull_work and push_work know where to push/pull.
    /// Stashes local changes if a different branch is selected as current.
    /// Also creates a PR from work_branch to destination_branch in the fork repo if all parameters are set.
    pub async fn pull_work(&self) -> Result<String, ZbobrError> {
        // Get required parameters
        let dest_repo = self.get_parameter(Parameter::DestinationRepository.name()).await?
            .ok_or_else(|| ZbobrError::Other("destination_repository parameter not set".to_string()))?;
        
        let dest_branch = self.get_parameter(Parameter::DestinationBranch.name()).await?
            .ok_or_else(|| ZbobrError::Other("destination_branch parameter not set".to_string()))?;

        let work_branch = self.get_parameter(Parameter::WorkBranch.name()).await?
            .ok_or_else(|| ZbobrError::Other("work_branch parameter not set".to_string()))?;

        // Clone and setup the repository with forking
        let path = self
            .zbobr
            .clone_and_setup(&dest_repo, &dest_branch, self.task_id)
            .await?;
        
        let path_str = path.to_string_lossy().to_string();

        // Track this repo for later push_work
        {
            let mut tracked = self.tracked_repos.lock().unwrap();
            tracked.insert(
                dest_repo.clone(),
                TrackedRepo {
                    repo: dest_repo.clone(),
                    local_path: path.clone(),
                },
            );
        } // Drop the guard here before any await

        // Create the work branch from destination_branch if it doesn't exist
        let create_branch = tokio::process::Command::new("git")
            .args(["checkout", "-b", &work_branch])
            .current_dir(&path)
            .status()
            .await?;

        if !create_branch.success() {
            // Branch might already exist, try to checkout
            let checkout_status = tokio::process::Command::new("git")
                .args(["checkout", &work_branch])
                .current_dir(&path)
                .status()
                .await?;

            if !checkout_status.success() {
                return Err(ZbobrError::Other(format!(
                    "Failed to create or checkout work branch '{}'",
                    work_branch
                )));
            }
        }

        // Clean up remote information - remove all remotes except origin
        tracing::info!("Cleaning up remote information in work repository");
        let remove_fork = tokio::process::Command::new("git")
            .args(["remote", "remove", "fork"])
            .current_dir(&path)
            .status()
            .await;

        // It's okay if fork doesn't exist
        if remove_fork.is_ok() && remove_fork.unwrap().success() {
            tracing::info!("Removed 'fork' remote");
        }

        // Create PR from work_branch to destination_branch in the fork repo
        if let Err(e) = self.create_pr_for_work_branch(&dest_repo, &work_branch, &dest_branch).await {
            // Log the error but don't fail the pull_work, let the worker know
            let hostname = hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string());
            let msg = format!("⚠️  Failed to create PR: {}. You can create it manually or continue working.", e);
            let _ = self.post_message(&msg, "worker", &hostname).await;
        }

        // Rename 'origin' to a temporary name, configure it with internal credentials, then back to origin
        // This ensures the model can't directly access remote URLs
        tracing::info!("Setting up internal remote for pull_work/push_work only");

        Ok(path_str)
    }

    /// Create a PR from work_branch to destination_branch in the fork repo.
    async fn create_pr_for_work_branch(
        &self,
        destination_repository: &str,
        work_branch: &str,
        destination_branch: &str,
    ) -> Result<(), ZbobrError> {
        tracing::info!(
            "Creating PR from {} to {} in fork",
            work_branch,
            destination_branch
        );

        let pr_url = self
            .zbobr
            .create_pr_in_fork(destination_repository, work_branch, destination_branch, self.task_id)
            .await?;

        // Store the PR URL in the task
        self.set_parameter(Parameter::PrUrl.name(), Some(pr_url)).await?;
        
        Ok(())
    }


    /// Mark task as done (sets signal to Done). Stage transition will be handled by main loop.
    pub async fn mark_done(&self) -> Result<(), ZbobrError> {
        self.set_signal(Signal::Done).await?;
        Ok(())
    }

    /// Get a task parameter value. Parameters are stored in the task's parameters HashMap.
    pub async fn get_parameter(&self, param_name: &str) -> Result<Option<String>, ZbobrError> {
        let task = self.zbobr.get_task(self.task_id).await?;
        
        // Try to match parameter name to Parameter enum
        let param = match param_name.to_lowercase().as_str() {
            name if name == Parameter::DestinationRepository.name() => Some(Parameter::DestinationRepository),
            name if name == Parameter::DestinationBranch.name() => Some(Parameter::DestinationBranch),
            name if name == Parameter::WorkBranch.name() => Some(Parameter::WorkBranch),
            name if name == Parameter::PrUrl.name() => Some(Parameter::PrUrl),
            _ => None,
        };
        
        if let Some(p) = param {
            return Ok(task.parameters.get(&p).cloned());
        }
        
        // If not a known parameter, extract from PARAMETERS section
        use crate::backend::extract_parameters;
        let parameters = extract_parameters(&task.description);
        Ok(parameters.get(param_name).cloned())
    }

    /// Set a task parameter value. Parameters are stored in the visible PARAMETERS section.
    pub async fn set_parameter(&self, param_name: &str, value: Option<String>) -> Result<(), ZbobrError> {
        use crate::backend::{parse_description_full, serialize_description_full};
        
        let task = self.zbobr.get_task(self.task_id).await?;
        let (description, mut parameters, plan, checklist) = parse_description_full(&task.description);
        
        // Update the parameter value
        let param_key = param_name.to_lowercase();
        if let Some(v) = value {
            parameters.insert(param_key, v);
        } else {
            parameters.remove(&param_key);
        }
        
        // Serialize back with updated parameters
        let body = serialize_description_full(&description, &parameters, &plan, &checklist);
        
        self.zbobr.update_task_description(self.task_id, &body).await
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_milestone_names() {
        assert_eq!(Stage::Pending.milestone_name(), "PENDING");
        assert_eq!(Stage::GoPlanning.milestone_name(), "GO_PLANNING");
        assert_eq!(Stage::Planning.milestone_name(), "PLANNING");
        assert_eq!(Stage::GoWorking.milestone_name(), "GO_WORKING");
        assert_eq!(Stage::Working.milestone_name(), "WORKING");
    }

    #[test]
    fn stage_display() {
        assert_eq!(Stage::Planning.to_string(), "PLANNING");
        assert_eq!(Stage::Working.to_string(), "WORKING");
    }

    #[test]
    fn stage_roundtrip_serde() {
        let stage = Stage::GoPlanning;
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
            plan: String::new(),
            discussion: vec!["Hello".to_string()],
            stage: Stage::Planning,
            tool: Some(Tool::Claude),
            model: Some(Model::Claude3Opus),
            parent_task_id: None,
            parameters: HashMap::new(),
            done: false,
            checklist: vec![],
            signal: None,
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

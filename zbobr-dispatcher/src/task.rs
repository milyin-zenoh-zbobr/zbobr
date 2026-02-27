use std::collections::HashMap;

use crate::Zbobr;

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

/// Robustly extract the repository name from a string (which could be a URL, local path, or owner/repo).
pub fn extract_repo_name(repo_ref: &str) -> Option<String> {
    let repo_ref = repo_ref.trim_end_matches(".git");
    let repo_ref = repo_ref.trim_end_matches('/');

    // Handle GitHub URLs or similar: https://github.com/owner/repo
    if repo_ref.contains("://") || repo_ref.starts_with("git@") {
        return repo_ref.split('/').next_back().map(|s| s.to_string());
    }

    // Handle local paths: /path/to/repo or ./repo
    if repo_ref.contains('/') {
        return repo_ref.split('/').next_back().map(|s| s.to_string());
    }

    // Fallback: just return the string if it doesn't contain slashes (already a repo name)
    Some(repo_ref.to_string())
}

// -- Checklist item types --

/// A single item in a task's checklist.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
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
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum Stage {
    Pending,
    Preparing,
    Planning,
    Working,
    Reviewing,
    Merging,
    Done,
}

impl Stage {
    pub fn milestone_name(&self) -> &'static str {
        match self {
            Stage::Pending => "PENDING",
            Stage::Preparing => "PREPARING",
            Stage::Planning => "PLANNING",
            Stage::Working => "WORKING",
            Stage::Reviewing => "REVIEWING",
            Stage::Merging => "MERGING",
            Stage::Done => "DONE",
        }
    }

    pub fn from_milestone_name(name: &str) -> Option<Self> {
        match name {
            "PENDING" => Some(Stage::Pending),
            "PREPARING" | "PREPARATION" => Some(Stage::Preparing),
            "PLANNING" => Some(Stage::Planning),
            "WORKING" => Some(Stage::Working),
            "REVIEWING" => Some(Stage::Reviewing),
            "MERGING" => Some(Stage::Merging),
            "DONE" => Some(Stage::Done),
            _ => None,
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.milestone_name())
    }
}

/// Role for task execution (planner, worker, reviewer, or merger).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum Role {
    #[serde(rename = "preparator")]
    Preparator,
    #[serde(rename = "planner")]
    Planner,
    #[serde(rename = "worker")]
    Worker,
    #[serde(rename = "reviewer")]
    Reviewer,
    #[serde(rename = "merger")]
    Merger,
}

impl Role {
    /// Returns the role name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Preparator => "preparator",
            Role::Planner => "planner",
            Role::Worker => "worker",
            Role::Reviewer => "reviewer",
            Role::Merger => "merger",
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Role {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "preparator" => Ok(Role::Preparator),
            "planner" => Ok(Role::Planner),
            "worker" => Ok(Role::Worker),
            "reviewer" => Ok(Role::Reviewer),
            "merger" => Ok(Role::Merger),
            _ => Err(anyhow::anyhow!("Unknown role: {}", s)),
        }
    }
}

/// Signal for task flow control (mapped to labels in GitHub backend).
/// Ordered by priority (highest to lowest): GoPrepare > GoPlan > GoWork > GoReview.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum Signal {
    #[serde(rename = "go_prepare")]
    GoPrepare = 1,
    #[serde(rename = "go_plan")]
    GoPlan = 2,
    #[serde(rename = "go_work")]
    GoWork = 3,
    #[serde(rename = "go_review")]
    GoReview = 4,
}

impl Signal {
    /// Returns the plain signal name.
    pub fn name(&self) -> &'static str {
        match self {
            Signal::GoReview => "go_review",
            Signal::GoWork => "go_work",
            Signal::GoPlan => "go_plan",
            Signal::GoPrepare => "go_prepare",
        }
    }

    /// Returns all available signals in priority order.
    pub fn all() -> &'static [Signal] {
        &[
            Signal::GoPrepare,
            Signal::GoPlan,
            Signal::GoWork,
            Signal::GoReview,
        ]
    }

    /// Maps signal to the role that should execute the session.
    pub fn target_role(&self) -> Role {
        match self {
            Signal::GoReview => Role::Reviewer,
            Signal::GoWork => Role::Worker,
            Signal::GoPlan => Role::Planner,
            Signal::GoPrepare => Role::Preparator,
        }
    }
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for Signal {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('_', "").as_str() {
            "goreview" | "go-review" => Ok(Signal::GoReview),
            "gowork" | "go-work" => Ok(Signal::GoWork),
            "goplan" | "go-plan" => Ok(Signal::GoPlan),
            "goprepare" | "go-prepare" => Ok(Signal::GoPrepare),
            _ => Err(anyhow::anyhow!("Unknown signal: {}", s)),
        }
    }
}

/// AI Tool/Agent to use.
#[derive(
    Debug,
    Clone,
    Copy,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    schemars::JsonSchema,
    Default,
)]
pub enum Tool {
    #[serde(rename = "copilot")]
    #[default]
    Copilot,
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "mcp-tester")]
    McpTester,
}

impl Tool {
    /// Returns all available tools.
    pub fn all() -> &'static [Tool] {
        &[Tool::Copilot, Tool::Claude, Tool::McpTester]
    }
}

impl std::fmt::Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tool::Copilot => write!(f, "copilot"),
            Tool::Claude => write!(f, "claude"),
            Tool::McpTester => write!(f, "mcp-tester"),
        }
    }
}

impl std::str::FromStr for Tool {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "copilot" => Ok(Tool::Copilot),
            "claude" => Ok(Tool::Claude),
            "mcp-tester" => Ok(Tool::McpTester),
            _ => Err(anyhow::anyhow!("Unknown tool: {}", s)),
        }
    }
}

/// AI Model to use.
#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, schemars::JsonSchema, Default,
)]
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
            Tool::McpTester => None,
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
    type Err = anyhow::Error;
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
            _ => Err(anyhow::anyhow!("Unknown model: {}", s)),
        }
    }
}

/// A task in the abstract domain (generic, backed by GitHub or Filesystem).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub plan: String,
    pub stage: Stage,
    pub tool: Option<Tool>,
    pub model: Option<Model>,
    pub parameters: HashMap<Parameter, String>,
    pub checklist: Vec<ChecklistItem>,
    pub signal: Option<Signal>,
    pub conflict: bool,
    pub pause: bool,
    /// When true the dispatcher will automatically set the pause flag any time
    /// the task's stage is changed.  This gives human operators an opportunity to
    /// review a transition before the next processing step occurs.
    pub confirm: bool,
    /// ETag for optimistic locking to prevent concurrent update conflicts.
    /// Used to detect if the task has been modified between read and write operations.
    #[serde(skip)]
    pub etag: Option<String>,
}

// ---------------------------------------------------------------------------
// RoleSession — restricted access for MCP tools during agent sessions.
//
// Cannot modify: stage, conflict, confirm (those are dispatcher-only transitions).
// Can modify: description, plan, checklist, parameters, signal, pause.
// ---------------------------------------------------------------------------

/// Restricted task session for MCP tool operations.
/// Stage and conflict flag are protected — only the dispatcher may change them.
#[derive(Clone)]
pub struct RoleSession {
    zbobr: Zbobr,
    task_id: u64,
}

impl RoleSession {
    pub(crate) fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self { zbobr, task_id }
    }

    pub fn task_id(&self) -> u64 {
        self.task_id
    }

    /// Create a branch name with the proper prefix for this task.
    pub fn create_branch_name(&self, short_name: &str) -> String {
        format!(
            "{}-{}-{}",
            self.zbobr.config().work_branch_prefix,
            self.task_id,
            short_name
        )
    }

    /// Check whether a branch name starts with this task's prefix.
    pub fn validate_branch_prefix(&self, branch: &str) -> bool {
        let prefix = format!(
            "{}-{}-",
            self.zbobr.config().work_branch_prefix,
            self.task_id
        );
        branch.starts_with(&prefix)
    }

    /// Read the full task state.
    pub async fn get_task(&self) -> anyhow::Result<Task> {
        self.zbobr.get_task(self.task_id).await
    }

    /// Get the current task description.
    pub async fn get_description(&self) -> anyhow::Result<String> {
        Ok(self.get_task().await?.description)
    }

    /// Get the current task plan.
    pub async fn get_plan(&self) -> anyhow::Result<String> {
        Ok(self.get_task().await?.plan)
    }

    /// Get the current task checklist.
    pub async fn get_checklist(&self) -> anyhow::Result<Vec<ChecklistItem>> {
        Ok(self.get_task().await?.checklist)
    }

    /// Atomically read-modify-write the task body.
    ///
    /// The closure receives a mutable `Task` reference and may modify `description`,
    /// `parameters`, `plan`, `checklist`, `signal`, and `pause`.
    ///
    /// **Protected fields**: `stage` and `conflict` are saved before the mutation
    /// and restored afterwards, so MCP tools cannot change them.
    pub async fn modify_task<F>(&self, mutate: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut Task) + Send + 'static,
    {
        let lock = self.zbobr.task_lock(self.task_id);
        let _guard = lock.lock().await;

        self.zbobr
            .task_backend
            .modify_task(
                self.task_id,
                Box::new(move |mut task| {
                    let saved_stage = task.stage;
                    let saved_conflict = task.conflict;
                    mutate(&mut task);
                    task.stage = saved_stage;
                    task.conflict = saved_conflict;
                    task
                }),
            )
            .await
    }

    /// Get all discussion messages on the task.
    pub async fn get_discussion(&self) -> anyhow::Result<Vec<String>> {
        self.zbobr.get_task_comments(self.task_id).await
    }

    /// Post a message to the task discussion with role and hostname metadata.
    pub async fn post_message(&self, msg: &str, role: &str, hostname: &str) -> anyhow::Result<()> {
        self.zbobr
            .post_task_comment(self.task_id, msg, role, hostname)
            .await
    }

    /// Get the current signal on the task.
    pub async fn get_signal(&self) -> anyhow::Result<Option<Signal>> {
        let task = self.zbobr.get_task(self.task_id).await?;
        Ok(task.signal)
    }

    /// Set signal on the task, respecting priority (higher priority signals cannot be overwritten by lower).
    pub async fn set_signal(&self, new_signal: Signal) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            // Only set if new signal has higher or equal priority (lower enum value)
            if let Some(current_signal) = task.signal
                && new_signal > current_signal
            {
                // new_signal has lower priority, don't overwrite
                return;
            }
            task.signal = Some(new_signal);
        })
        .await
    }

    /// Clear the signal on the task.
    pub async fn clear_signal(&self) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            task.signal = None;
        })
        .await
    }

    /// Set the pause flag on the task.
    pub async fn set_pause(&self, pause: bool) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            task.pause = pause;
        })
        .await
    }

    /// Clone target repo and checkout specific branch (read-only, for planner).
    pub async fn request_branch_readonly(
        &self,
        repo: &str,
        branch: &str,
    ) -> anyhow::Result<String> {
        let path = self
            .zbobr
            .clone_readonly(repo, branch, self.task_id)
            .await?;
        let path_str = path.to_string_lossy().to_string();
        Ok(path_str)
    }

    /// Fork target repo, clone locally, checkout specific branch (for worker).
    pub async fn request_branch(&self, repo: &str, branch: &str) -> anyhow::Result<String> {
        let task = self.zbobr.get_task(self.task_id).await?;
        let destination_branch = task
            .parameters
            .get(&crate::Parameter::DestinationBranch)
            .cloned()
            .unwrap_or_else(|| "main".to_string());
        let path = self
            .zbobr
            .clone_and_setup(repo, branch, &destination_branch, self.task_id)
            .await?;
        let path_str = path.to_string_lossy().to_string();
        Ok(path_str)
    }

    /// Helper: Clone repo and checkout branch from PR.
    /// PR format: "https://github.com/owner/repo/pull/123" or "owner/repo#123"
    pub async fn request_branch_by_pr(&self, pr: &str, readonly: bool) -> anyhow::Result<String> {
        let (repo, branch) = self.zbobr.parse_pr_to_repo_branch(pr).await?;
        if readonly {
            self.request_branch_readonly(&repo, &branch).await
        } else {
            self.request_branch(&repo, &branch).await
        }
    }

    /// Push the current branch to the fork remote.
    /// Validates that the current branch has the correct task prefix.
    pub async fn push_branch(&self, path: &str) -> anyhow::Result<()> {
        let work_dir = std::path::PathBuf::from(path);

        if !work_dir.exists() {
            anyhow::bail!("Work directory does not exist: {}", work_dir.display());
        }

        // Get current branch name
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&work_dir)
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("Failed to get current branch");
        }

        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if !self.validate_branch_prefix(&current_branch) {
            anyhow::bail!(
                "Branch '{}' does not match expected prefix '{}/{}/'. Use create_branch_name to generate a valid branch name.",
                current_branch,
                self.zbobr.config().work_branch_prefix,
                self.task_id
            );
        }

        // Push to fork
        tracing::info!("Pushing branch '{}' to fork", current_branch);
        let status = tokio::process::Command::new("git")
            .args(["push", "-u", "fork", "HEAD", "--force"])
            .current_dir(&work_dir)
            .status()
            .await?;

        if !status.success() {
            anyhow::bail!("Failed to push to fork");
        }

        Ok(())
    }

    /// Push the branch and create PR within the fork.
    /// The PR is created in the fork repo with `destination_branch` as base.
    pub async fn push_branch_and_create_pr(
        &self,
        path: &str,
        destination_branch: &str,
    ) -> anyhow::Result<String> {
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

        // Derive repository name from work directory name (workspace/task#/repo)
        let repo_name = work_dir
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Could not determine repo name from path: {}", path))?
            .to_string();

        // Build PR metadata from task (decoupled from repo backend)
        let task = self.get_task().await?;
        let pr_title = format!("Fix #{}: {}", self.task_id, task.title);
        let pr_body = format!(
            "Resolves #{}\n\nImplementation for: {}",
            self.task_id, task.title
        );

        // Create PR using the backend (which knows the fork owner)
        let pr_url = self
            .zbobr
            .create_pr_in_fork(
                &repo_name,
                &current_branch,
                destination_branch,
                &pr_title,
                &pr_body,
            )
            .await?;
        Ok(pr_url)
    }

    /// Ensure `pr_url` is stored in task parameters.
    ///
    /// If already set, returns the existing value immediately.
    /// If not set: calls `ensure_branch_and_pr` on the repo backend, stores the
    /// resulting URL in `Parameter::PrUrl`, and returns it.
    pub async fn ensure_pr_url(&self) -> anyhow::Result<String> {
        let task = self.get_task().await?;
        if let Some(url) = task.parameters.get(&Parameter::PrUrl).cloned() {
            return Ok(url);
        }
        let dest_repo = task
            .parameters
            .get(&Parameter::DestinationRepository)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("destination_repository parameter is not set"))?;
        let dest_branch = task
            .parameters
            .get(&Parameter::DestinationBranch)
            .cloned()
            .unwrap_or_else(|| "main".to_string());
        let work_branch = task
            .parameters
            .get(&Parameter::WorkBranch)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("work_branch parameter is not set"))?;
        let pr_title = format!("Fix #{}: {}", self.task_id, task.title);

        let pr_url = self
            .zbobr
            .ensure_branch_and_pr(
                &dest_repo,
                self.task_id,
                &work_branch,
                &dest_branch,
                &pr_title,
            )
            .await?;

        self.set_parameter(Parameter::PrUrl, Some(pr_url.clone()))
            .await?;
        Ok(pr_url)
    }

    /// Push current work branch commits to the remote.
    ///
    /// Reads `DestinationRepository` and `WorkBranch` from task parameters and delegates
    /// to the repo backend. FS backend is a no-op; GitHub backend performs a git push.
    pub async fn push_branch_commits(&self) -> anyhow::Result<()> {
        let task = self.get_task().await?;
        let dest_repo = task
            .parameters
            .get(&Parameter::DestinationRepository)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("destination_repository parameter is not set"))?;
        let work_branch = task
            .parameters
            .get(&Parameter::WorkBranch)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("work_branch parameter is not set"))?;
        self.zbobr
            .push_branch(&dest_repo, self.task_id, &work_branch)
            .await
    }

    /// Get a task parameter value. Parameters are stored in the task's parameters HashMap.
    pub async fn get_parameter(&self, param: Parameter) -> anyhow::Result<Option<String>> {
        let task = self.zbobr.get_task(self.task_id).await?;
        Ok(task.parameters.get(&param).cloned())
    }

    /// Set a task parameter value with automatic conflict detection.
    /// Parameters are stored in the visible PARAMETERS section.
    pub async fn set_parameter(
        &self,
        param: Parameter,
        value: Option<String>,
    ) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            if let Some(v) = value {
                task.parameters.insert(param, v);
            } else {
                task.parameters.remove(&param);
            }
        })
        .await
    }
}

// ---------------------------------------------------------------------------
// TaskSession — full access for the dispatcher.
//
// Can modify everything including stage and conflict flag.
// ---------------------------------------------------------------------------

/// Full-access task session for the dispatcher.
/// Can change stage, conflict flag, and all other fields.
#[derive(Clone)]
pub struct TaskSession {
    zbobr: Zbobr,
    task_id: u64,
}

impl TaskSession {
    pub(crate) fn new(zbobr: Zbobr, task_id: u64) -> Self {
        Self { zbobr, task_id }
    }

    pub fn task_id(&self) -> u64 {
        self.task_id
    }

    /// Get a restricted RoleSession view for MCP tool operations.
    pub fn role_session(&self) -> RoleSession {
        RoleSession::new(self.zbobr.clone(), self.task_id)
    }

    /// Read the full task state.
    pub async fn get_task(&self) -> anyhow::Result<Task> {
        self.zbobr.get_task(self.task_id).await
    }

    /// Get the current task checklist.
    pub async fn get_checklist(&self) -> anyhow::Result<Vec<ChecklistItem>> {
        Ok(self.get_task().await?.checklist)
    }

    /// Atomically read-modify-write the task with unrestricted access.
    /// Only the dispatcher should use this.
    pub async fn modify_task<F>(&self, mutate: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut Task) + Send + 'static,
    {
        let lock = self.zbobr.task_lock(self.task_id);
        let _guard = lock.lock().await;

        self.zbobr
            .task_backend
            .modify_task(
                self.task_id,
                Box::new(move |mut task| {
                    mutate(&mut task);
                    task
                }),
            )
            .await
    }

    /// Set the task stage (dispatcher only).
    pub async fn set_stage(&self, stage: Stage) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            // if confirm flag is enabled we always pause on any stage transition
            if task.confirm && task.stage != stage {
                task.pause = true;
            }
            task.stage = stage;
        })
        .await
    }

    /// Set the confirm flag on the task (dispatcher only).
    /// This is convenience used by create_task_with_confirm.
    pub async fn set_confirm(&self, confirm: bool) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            task.confirm = confirm;
        })
        .await
    }

    /// Set the conflict flag (dispatcher only).
    pub async fn set_conflict(&self, conflict: bool) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            task.conflict = conflict;
        })
        .await
    }

    /// Set signal on the task (dispatcher only, no priority check).
    pub async fn set_signal(&self, signal: Option<Signal>) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            task.signal = signal;
        })
        .await
    }

    /// Mark task as done: set stage to Done and clear signal.
    pub async fn mark_done(&self) -> anyhow::Result<()> {
        self.modify_task(move |task| {
            task.stage = Stage::Done;
            task.signal = None;
        })
        .await
    }

    /// Post a message to the task discussion with role and hostname metadata.
    pub async fn post_message(&self, msg: &str, role: &str, hostname: &str) -> anyhow::Result<()> {
        self.zbobr
            .post_task_comment(self.task_id, msg, role, hostname)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    #[test]
    fn stage_milestone_names() {
        assert_eq!(Stage::Pending.milestone_name(), "PENDING");
        assert_eq!(Stage::Planning.milestone_name(), "PLANNING");
        assert_eq!(Stage::Working.milestone_name(), "WORKING");
        assert_eq!(Stage::Reviewing.milestone_name(), "REVIEWING");
        assert_eq!(Stage::Preparing.milestone_name(), "PREPARING");
        assert_eq!(Stage::Merging.milestone_name(), "MERGING");
        assert_eq!(Stage::Done.milestone_name(), "DONE");
    }

    #[test]
    fn stage_backward_compat() {
        assert_eq!(
            Stage::from_milestone_name("PREPARATION"),
            Some(Stage::Preparing)
        );
        assert_eq!(
            Stage::from_milestone_name("PREPARING"),
            Some(Stage::Preparing)
        );
    }

    #[test]
    fn stage_display() {
        assert_eq!(Stage::Planning.to_string(), "PLANNING");
        assert_eq!(Stage::Working.to_string(), "WORKING");
        assert_eq!(Stage::Reviewing.to_string(), "REVIEWING");
        assert_eq!(Stage::Done.to_string(), "DONE");
    }

    #[test]
    fn stage_roundtrip_serde() {
        let stage = Stage::Planning;
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
            stage: Stage::Planning,
            tool: Some(Tool::Claude),
            model: Some(Model::Claude3Opus),
            parameters: HashMap::new(),
            checklist: vec![],
            signal: None,
            conflict: false,
            pause: false,
            confirm: false,
            etag: None,
        };
        let json = serde_json::to_string(&task).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 42);
        assert_eq!(back.title, "Test task");
        assert_eq!(back.stage, Stage::Planning);
        assert_eq!(back.tool, Some(Tool::Claude));
        assert_eq!(back.model, Some(Model::Claude3Opus));
        assert!(!back.conflict);
        assert!(!back.pause);
        assert!(!back.confirm);
    }

    #[test]
    fn confirm_flag_triggers_pause_on_stage_change() {
        let mut t = Task {
            id: 1,
            title: "x".into(),
            description: "".into(),
            plan: String::new(),
            stage: Stage::Pending,
            tool: None,
            model: None,
            parameters: HashMap::new(),
            checklist: vec![],
            signal: None,
            conflict: false,
            pause: false,
            confirm: true,
            etag: None,
        };
        let new_stage = Stage::Planning;
        if t.confirm && t.stage != new_stage {
            t.pause = true;
        }
        t.stage = new_stage;
        assert!(t.pause, "task should have been paused when confirm=true and stage changed");
    }

    #[test]
    fn update_closure_applies_confirm_first() {
        // simulate the modify_task closure used by the CLI update command
        let mut task = Task {
            id: 2,
            title: "x".into(),
            description: "".into(),
            plan: String::new(),
            stage: Stage::Pending,
            tool: None,
            model: None,
            parameters: HashMap::new(),
            checklist: vec![],
            signal: None,
            conflict: false,
            pause: false,
            confirm: false,
            etag: None,
        };
        let new_confirm = Some(true);
        let new_stage = Some(Stage::Planning);

        if let Some(c) = new_confirm {
            task.confirm = c;
        }
        if let Some(s) = new_stage {
            if task.confirm && task.stage != s {
                task.pause = true;
            }
            task.stage = s;
        }

        assert!(task.pause, "pause must be set when confirm is enabled and stage changes");
    }

    // asynchronous tests require a runtime; use tokio for the following
    #[tokio::test]
    async fn task_session_set_stage_pauses_when_confirm() {
        use crate::{Zbobr, config::ZbobrDispatcherConfig};
        use std::sync::Arc;
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicU64, Ordering};
        use tokio::sync::Mutex;

        // simple in-memory task backend implementation for testing
        struct DummyBackend {
            tasks: Mutex<HashMap<u64, Task>>,
            next_id: AtomicU64,
        }

        #[async_trait]
        impl crate::backend::TaskBackend for DummyBackend {
            async fn get_task(&self, id: u64) -> anyhow::Result<Task> {
                let tasks = self.tasks.lock().await;
                tasks
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("not found"))
            }
            async fn create_task(
                &self,
                title: &str,
                description: &str,
                stage: Stage,
                tool: Option<Tool>,
                model: Option<Model>,
                parameters: HashMap<Parameter, String>,
            ) -> anyhow::Result<u64> {
                let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
                let task = Task {
                    id,
                    title: title.to_string(),
                    description: description.to_string(),
                    plan: String::new(),
                    stage,
                    tool,
                    model,
                    parameters,
                    checklist: vec![],
                    signal: None,
                    conflict: false,
                    pause: false,
                    confirm: false,
                    etag: None,
                };
                self.tasks.lock().await.insert(id, task);
                Ok(id)
            }
            async fn close_task(&self, _id: u64) -> anyhow::Result<()> { Ok(()) }
            async fn is_task_closed(&self, _id: u64) -> anyhow::Result<bool> { Ok(false) }
            async fn modify_task(
                &self,
                id: u64,
                mutate: Box<dyn FnOnce(Task) -> Task + Send>,
            ) -> anyhow::Result<()> {
                let mut tasks = self.tasks.lock().await;
                if let Some(t) = tasks.remove(&id) {
                    let t = mutate(t);
                    tasks.insert(id, t);
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("not found"))
                }
            }
            async fn list_tasks_by_stage(
                &self,
                _stage: Stage,
                _tool: Option<Tool>,
            ) -> anyhow::Result<Vec<Task>> {
                Ok(vec![])
            }
            async fn get_task_comments(&self, _id: u64) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            async fn post_task_comment(
                &self,
                _id: u64,
                _body: &str,
                _role: &str,
                _hostname: &str,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            async fn setup(&self, _force: bool) -> anyhow::Result<()> { Ok(()) }
            async fn validate_connectivity(&self) -> anyhow::Result<()> { Ok(()) }
        }

        struct DummyRepo;
        #[async_trait]
        impl crate::backend::RepoBackend for DummyRepo {
            async fn clone_and_setup(
                &self,
                _target_repo: &str,
                _work_branch: &str,
                _destination_branch: &str,
                _workspace_path: &std::path::Path,
            ) -> anyhow::Result<std::path::PathBuf> {
                unreachable!()
            }
            async fn clone_readonly(
                &self,
                _target_repo: &str,
                _branch: &str,
                _workspace_path: &std::path::Path,
            ) -> anyhow::Result<std::path::PathBuf> {
                unreachable!()
            }
            async fn setup_fork_remote_and_push(
                &self,
                _work_dir: &std::path::Path,
                _target_repo: &str,
                _work_branch: &str,
            ) -> anyhow::Result<()> {
                unreachable!()
            }
            async fn ensure_branch_and_pr(
                &self,
                _target_repo: &str,
                _workspace_path: &std::path::Path,
                _work_branch: &str,
                _destination_branch: &str,
                _pr_title: &str,
            ) -> anyhow::Result<String> {
                unreachable!()
            }
            async fn push_branch(
                &self,
                _target_repo: &str,
                _workspace_path: &std::path::Path,
                _work_branch: &str,
            ) -> anyhow::Result<()> {
                unreachable!()
            }
            async fn create_pr_in_fork(
                &self,
                _repo_name: &str,
                _work_branch: &str,
                _destination_branch: &str,
                _pr_title: &str,
                _pr_body: &str,
            ) -> anyhow::Result<String> {
                unreachable!()
            }
            async fn parse_pr_to_repo_branch(&self, _pr_ref: &str) -> anyhow::Result<(String, String)> {
                unreachable!()
            }
            async fn validate_connectivity(&self) -> anyhow::Result<()> {
                Ok(())
            }
            fn debug_state(&self) -> String { "dummy".to_string() }
        }

        let backend = Arc::new(DummyBackend {
            tasks: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(0),
        });
        let repo = Arc::new(DummyRepo);
        let zbobr = Zbobr::new(ZbobrDispatcherConfig::default(), backend.clone(), repo.clone());

        let id = zbobr
            .create_task("t", "", Stage::Pending, None, None, None, None)
            .await
            .unwrap();
        zbobr.task_session(id).set_confirm(true).await.unwrap();
        zbobr.task_session(id).set_stage(Stage::Planning).await.unwrap();
        let t = zbobr.get_task(id).await.unwrap();
        assert!(t.pause);
    }

    #[test]
    fn signal_target_role() {
        assert_eq!(Signal::GoPrepare.target_role(), Role::Preparator);
        assert_eq!(Signal::GoPlan.target_role(), Role::Planner);
        assert_eq!(Signal::GoWork.target_role(), Role::Worker);
        assert_eq!(Signal::GoReview.target_role(), Role::Reviewer);
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

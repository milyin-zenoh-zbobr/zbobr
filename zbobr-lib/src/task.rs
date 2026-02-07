use crate::{Zbobr, ZbobrError};

/// Workflow stage (maps to GitHub milestones internally).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Stage {
    Planning,
    Pending,
    Ready,
    Working,
}

impl Stage {
    pub fn milestone_name(&self) -> &'static str {
        match self {
            Stage::Planning => "PLANNING",
            Stage::Pending => "PENDING",
            Stage::Ready => "READY",
            Stage::Working => "WORKING",
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.milestone_name())
    }
}

/// A task in the abstract domain (backed by a GitHub issue).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub stage: Stage,
    pub model: Option<String>,
    pub done: bool,
}

/// Planner session bound to a specific task.
/// Copilot agents interact through this -- no GitHub concepts exposed.
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

    /// Get the current plan text (issue body).
    pub async fn get_plan(&self) -> Result<String, ZbobrError> {
        let task = self.zbobr.get_issue(self.task_id).await?;
        Ok(task.description)
    }

    /// Update the plan text (replace issue body).
    pub async fn set_plan(&self, plan: &str) -> Result<(), ZbobrError> {
        self.zbobr.update_issue_body(self.task_id, plan).await
    }

    /// Get all discussion messages on the task.
    pub async fn get_discussion(&self) -> Result<Vec<String>, ZbobrError> {
        self.zbobr.get_issue_comments(self.task_id).await
    }

    /// Post a message to the task discussion.
    pub async fn post_message(&self, msg: &str) -> Result<(), ZbobrError> {
        self.zbobr.post_issue_comment(self.task_id, msg).await
    }

    /// Clone target repo locally for investigation (read-only).
    pub async fn request_repo(&self, repo: &str) -> Result<String, ZbobrError> {
        let path = self.zbobr.clone_readonly(repo, self.task_id).await?;
        Ok(path.to_string_lossy().to_string())
    }
}

/// Worker session bound to a specific task.
/// Copilot agents interact through this -- no GitHub concepts exposed.
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

    /// Get the current plan text.
    pub async fn get_plan(&self) -> Result<String, ZbobrError> {
        let task = self.zbobr.get_issue(self.task_id).await?;
        Ok(task.description)
    }

    /// Get all discussion messages.
    pub async fn get_discussion(&self) -> Result<Vec<String>, ZbobrError> {
        self.zbobr.get_issue_comments(self.task_id).await
    }

    /// Post a message to the task discussion.
    pub async fn post_message(&self, msg: &str) -> Result<(), ZbobrError> {
        self.zbobr.post_issue_comment(self.task_id, msg).await
    }

    /// Fork target repo, clone locally, create branch, return local path.
    pub async fn request_repo(&self, repo: &str) -> Result<String, ZbobrError> {
        let path = self.zbobr.clone_and_setup(repo, self.task_id).await?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Push changes and create PR from the prepared branch.
    pub async fn submit_work(&self, target_repo: &str) -> Result<String, ZbobrError> {
        self.zbobr.push_and_create_pr(target_repo, self.task_id).await
    }

    /// Mark task as done (sets done flag and transitions to Pending).
    pub async fn mark_done(&self) -> Result<(), ZbobrError> {
        self.zbobr
            .set_issue_milestone(self.task_id, Stage::Pending.milestone_name())
            .await?;
        self.zbobr
            .add_issue_label(self.task_id, "done")
            .await?;
        Ok(())
    }
}

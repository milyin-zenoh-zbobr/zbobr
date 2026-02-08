use crate::{Zbobr, ZbobrError};

/// Workflow stage (maps to GitHub milestones internally).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
        self.zbobr
            .push_and_create_pr(target_repo, self.task_id)
            .await
    }

    /// Mark task as done (sets done flag and transitions to Pending).
    pub async fn mark_done(&self) -> Result<(), ZbobrError> {
        self.zbobr
            .set_issue_milestone(self.task_id, Stage::Pending.milestone_name())
            .await?;
        self.zbobr.add_issue_label(self.task_id, "done").await?;
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
            stage: Stage::Planning,
            model: Some("claude-opus-4-6".to_string()),
            done: false,
        };
        let json = serde_json::to_string(&task).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 42);
        assert_eq!(back.title, "Test task");
        assert_eq!(back.stage, Stage::Planning);
        assert_eq!(back.model, Some("claude-opus-4-6".to_string()));
        assert!(!back.done);
    }
}

use crate::{Stage, Task, Zbobr, ZbobrError};

impl Zbobr {
    /// Find all tasks in a given stage.
    pub async fn find_tasks_by_stage(&self, stage: Stage) -> Result<Vec<Task>, ZbobrError> {
        self.list_issues_by_milestone(stage.milestone_name()).await
    }

    /// Set a task's stage (milestone).
    pub async fn set_task_stage(&self, task_id: u64, stage: Stage) -> Result<(), ZbobrError> {
        self.set_issue_milestone(task_id, stage.milestone_name())
            .await
    }

    /// Find all tasks in a given stage (by name).
    pub async fn find_tasks_by_stage_name(&self, name: &str) -> Result<Vec<Task>, ZbobrError> {
        self.list_issues_by_milestone(name).await
    }

    /// Set a task's stage (by name).
    pub async fn set_task_stage_name(&self, task_id: u64, name: &str) -> Result<(), ZbobrError> {
        self.set_issue_milestone(task_id, name).await
    }
}

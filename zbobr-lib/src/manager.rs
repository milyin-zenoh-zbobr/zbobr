use crate::{Stage, Task, Zbobr, ZbobrError};

impl Zbobr {
    /// Find all tasks in a given stage.
    pub async fn find_tasks_by_stage(&self, stage: Stage) -> Result<Vec<Task>, ZbobrError> {
        self.list_tasks_by_stage(stage.milestone_name(), None).await
    }

    /// Set a task's stage (milestone).
    pub async fn set_task_stage(&self, task_id: u64, stage: Stage) -> Result<(), ZbobrError> {
        self.set_task_stage_by_name(task_id, stage.milestone_name())
            .await
    }

    /// Find all tasks in a given stage (by name).
    pub async fn find_tasks_by_stage_name(&self, name: &str) -> Result<Vec<Task>, ZbobrError> {
        self.list_tasks_by_stage(name, None).await
    }
}

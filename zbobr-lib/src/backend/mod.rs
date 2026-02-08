pub mod github;
pub mod stub;

use crate::{Task, ZbobrError};
use async_trait::async_trait;
use std::path::PathBuf;

#[async_trait]
pub trait Backend: Send + Sync {
    /// Get an issue as a Task.
    async fn get_issue(&self, issue_number: u64) -> Result<Task, ZbobrError>;

    /// Get all comments on an issue as formatted discussion.
    async fn get_issue_comments(&self, issue_number: u64) -> Result<Vec<String>, ZbobrError>;

    /// Post a comment on an issue.
    async fn post_issue_comment(&self, issue_number: u64, body: &str) -> Result<(), ZbobrError>;

    /// Set the milestone on an issue by milestone title.
    async fn set_issue_milestone(
        &self,
        issue_number: u64,
        milestone_title: &str,
    ) -> Result<(), ZbobrError>;

    /// Add a label to an issue.
    async fn add_issue_label(&self, issue_number: u64, label: &str) -> Result<(), ZbobrError>;

    /// Remove a label from an issue.
    async fn remove_issue_label(&self, issue_number: u64, label: &str) -> Result<(), ZbobrError>;

    /// Update the issue body (description).
    async fn update_issue_body(&self, issue_number: u64, body: &str) -> Result<(), ZbobrError>;

    /// List open issues with a given milestone title.
    async fn list_issues_by_milestone(
        &self,
        milestone_title: &str,
    ) -> Result<Vec<Task>, ZbobrError>;

    /// Check if an issue is closed.
    async fn is_issue_closed(&self, issue_number: u64) -> Result<bool, ZbobrError>;

    /// Check if a file exists in the domain repo.
    async fn repo_file_exists(&self, path: &str) -> Result<bool, ZbobrError>;

    /// Create or update a file in the domain repo.
    async fn create_repo_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<(), ZbobrError>;

    /// Ensure the domain repo exists.
    async fn ensure_domain_repo_exists(&self) -> Result<(), ZbobrError>;

    /// Clone a repo into the workspace, set up fork remote and feature branch.
    /// Returns the local path.
    async fn clone_and_setup(&self, target_repo: &str, task_id: u64)
        -> Result<PathBuf, ZbobrError>;

    /// Clone a repo for read-only investigation (no fork, no branch).
    async fn clone_readonly(&self, target_repo: &str, task_id: u64) -> Result<PathBuf, ZbobrError>;

    /// Push the current branch to the fork remote and create a PR.
    async fn push_and_create_pr(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError>;

    // -- Setup methods --

    /// List all milestones in the domain repo.
    async fn list_milestones(&self) -> Result<Vec<(u64, String)>, ZbobrError>;

    /// Create a milestone in the domain repo.
    async fn create_milestone(&self, title: &str, description: &str) -> Result<(), ZbobrError>;

    /// Delete a milestone by its number.
    async fn delete_milestone(&self, number: u64) -> Result<(), ZbobrError>;

    /// List all labels in the domain repo.
    async fn list_labels(&self) -> Result<Vec<String>, ZbobrError>;

    /// Create a label in the domain repo.
    async fn create_label(
        &self,
        name: &str,
        color: &str,
        description: &str,
    ) -> Result<(), ZbobrError>;
}

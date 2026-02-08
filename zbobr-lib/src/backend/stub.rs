use super::Backend;
use crate::{Stage, Task, ZbobrError};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
struct StubState {
    issues: HashMap<u64, Task>,
    comments: HashMap<u64, Vec<String>>,
    milestones: HashMap<String, u64>, // title -> number
    labels: HashSet<String>,
    files: HashMap<String, String>, // path -> content
    next_issue_id: u64,
}

impl Default for StubState {
    fn default() -> Self {
        let mut issues = HashMap::new();
        // Add a sample issue
        issues.insert(
            1,
            Task {
                id: 1,
                title: "Stub Issue".to_string(),
                description: "This is a stub issue for testing.".to_string(),
                stage: Stage::Pending,
                model: None,
                done: false,
            },
        );

        let mut milestones = HashMap::new();
        milestones.insert(Stage::Pending.milestone_name().to_string(), 1);
        milestones.insert(Stage::PlanningReady.milestone_name().to_string(), 2);
        milestones.insert(Stage::Planning.milestone_name().to_string(), 3);
        milestones.insert(Stage::WorkingReady.milestone_name().to_string(), 4);
        milestones.insert(Stage::Working.milestone_name().to_string(), 5);

        Self {
            issues,
            comments: HashMap::new(),
            milestones,
            labels: HashSet::new(),
            files: HashMap::new(),
            next_issue_id: 2,
        }
    }
}

pub struct StubBackend {
    state: Arc<RwLock<StubState>>,
    workspace: PathBuf,
}

impl StubBackend {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            state: Arc::new(RwLock::new(StubState::default())),
            workspace,
        }
    }
}

#[async_trait]
impl Backend for StubBackend {
    async fn get_issue(&self, issue_number: u64) -> Result<Task, ZbobrError> {
        let state = self.state.read().unwrap();
        state
            .issues
            .get(&issue_number)
            .cloned()
            .ok_or_else(|| ZbobrError::GitHub(format!("Issue {issue_number} not found")))
    }

    async fn get_issue_comments(&self, issue_number: u64) -> Result<Vec<String>, ZbobrError> {
        let state = self.state.read().unwrap();
        Ok(state
            .comments
            .get(&issue_number)
            .cloned()
            .unwrap_or_default())
    }

    async fn post_issue_comment(&self, issue_number: u64, body: &str) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        let comments = state.comments.entry(issue_number).or_default();
        comments.push(format!("stub-user: {body}"));
        Ok(())
    }

    async fn set_issue_milestone(
        &self,
        issue_number: u64,
        milestone_title: &str,
    ) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        if let Some(task) = state.issues.get_mut(&issue_number) {
            let stage = match milestone_title {
                "PENDING" => Stage::Pending,
                "PLANNING_READY" => Stage::PlanningReady,
                "PLANNING" => Stage::Planning,
                "WORKING_READY" => Stage::WorkingReady,
                "WORKING" => Stage::Working,
                _ => {
                    return Err(ZbobrError::GitHub(format!(
                        "Unknown milestone {milestone_title}"
                    )))
                }
            };
            task.stage = stage;
            Ok(())
        } else {
            Err(ZbobrError::GitHub(format!(
                "Issue {issue_number} not found"
            )))
        }
    }

    async fn add_issue_label(&self, issue_number: u64, label: &str) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        if let Some(task) = state.issues.get_mut(&issue_number) {
            if label == "done" {
                task.done = true;
            } else if let Some(model) = label.strip_prefix("copilot:") {
                task.model = Some(model.to_string());
            }
            Ok(())
        } else {
            Err(ZbobrError::GitHub(format!(
                "Issue {issue_number} not found"
            )))
        }
    }

    async fn remove_issue_label(&self, issue_number: u64, label: &str) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        if let Some(task) = state.issues.get_mut(&issue_number) {
            if label == "done" {
                task.done = false;
            }
            // For model, we can't easily remove it without storing all labels separately,
            // but for stub purposes this might be enough.
            Ok(())
        } else {
            Err(ZbobrError::GitHub(format!(
                "Issue {issue_number} not found"
            )))
        }
    }

    async fn update_issue_body(&self, issue_number: u64, body: &str) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        if let Some(task) = state.issues.get_mut(&issue_number) {
            task.description = body.to_string();
            Ok(())
        } else {
            Err(ZbobrError::GitHub(format!(
                "Issue {issue_number} not found"
            )))
        }
    }

    async fn list_issues_by_milestone(
        &self,
        milestone_title: &str,
    ) -> Result<Vec<Task>, ZbobrError> {
        let state = self.state.read().unwrap();
        let target_stage = match milestone_title {
            "PENDING" => Stage::Pending,
            "PLANNING_READY" => Stage::PlanningReady,
            "PLANNING" => Stage::Planning,
            "WORKING_READY" => Stage::WorkingReady,
            "WORKING" => Stage::Working,
            _ => return Ok(vec![]),
        };

        Ok(state
            .issues
            .values()
            .filter(|t| t.stage == target_stage)
            .cloned()
            .collect())
    }

    async fn is_issue_closed(&self, issue_number: u64) -> Result<bool, ZbobrError> {
        let state = self.state.read().unwrap();
        Ok(!state.issues.contains_key(&issue_number))
    }

    async fn repo_file_exists(&self, path: &str) -> Result<bool, ZbobrError> {
        let state = self.state.read().unwrap();
        Ok(state.files.contains_key(path))
    }

    async fn create_repo_file(
        &self,
        path: &str,
        content: &str,
        _commit_message: &str,
    ) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        state.files.insert(path.to_string(), content.to_string());
        Ok(())
    }

    async fn ensure_domain_repo_exists(&self) -> Result<(), ZbobrError> {
        Ok(())
    }

    async fn clone_and_setup(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        let repo_name = target_repo.split('/').nth(1).unwrap_or(target_repo);
        let issue_dir = self.workspace.join(format!("issue#{task_id}"));
        let work_dir = issue_dir.join(repo_name);

        tokio::fs::create_dir_all(&work_dir).await?;

        // Initialize git repo so agents can run git commands
        if !work_dir.join(".git").exists() {
            tokio::process::Command::new("git")
                .args(["init"])
                .current_dir(&work_dir)
                .status()
                .await?;

            // Commit an initial file
            tokio::fs::write(work_dir.join("README.md"), "# Stub Repo").await?;
            tokio::process::Command::new("git")
                .args(["add", "."])
                .current_dir(&work_dir)
                .status()
                .await?;
            tokio::process::Command::new("git")
                .args(["commit", "-m", "Initial commit"])
                .current_dir(&work_dir)
                .status()
                .await?;
        }

        Ok(work_dir)
    }

    async fn clone_readonly(&self, target_repo: &str, task_id: u64) -> Result<PathBuf, ZbobrError> {
        // reuse clone_and_setup logic for stub
        self.clone_and_setup(target_repo, task_id).await
    }

    async fn push_and_create_pr(
        &self,
        _target_repo: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError> {
        Ok(format!("https://github.com/stub/repo/pull/{task_id}"))
    }

    async fn list_milestones(&self) -> Result<Vec<(u64, String)>, ZbobrError> {
        let state = self.state.read().unwrap();
        Ok(state
            .milestones
            .iter()
            .map(|(k, v)| (*v, k.clone()))
            .collect())
    }

    async fn create_milestone(&self, title: &str, _description: &str) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        let id = state.milestones.len() as u64 + 1;
        state.milestones.insert(title.to_string(), id);
        Ok(())
    }

    async fn delete_milestone(&self, number: u64) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        state.milestones.retain(|_, v| *v != number);
        Ok(())
    }

    async fn list_labels(&self) -> Result<Vec<String>, ZbobrError> {
        let state = self.state.read().unwrap();
        Ok(state.labels.iter().cloned().collect())
    }

    async fn create_label(
        &self,
        name: &str,
        _color: &str,
        _description: &str,
    ) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        state.labels.insert(name.to_string());
        Ok(())
    }
}

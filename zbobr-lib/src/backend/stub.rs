use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;

use super::Backend;
use crate::{Label, Model, Stage, Task, Tool, ZbobrError};

#[derive(Debug)]
struct StubState {
    tasks: HashMap<u64, Task>,
    comments: HashMap<u64, Vec<String>>,
    stages: HashMap<String, u64>, // title -> number
    labels: HashSet<String>,
    files: HashMap<String, String>, // path -> content
    next_task_id: u64,
}

impl Default for StubState {
    fn default() -> Self {
        let mut tasks = HashMap::new();
        // Add a sample task
        tasks.insert(
            1,
            Task {
                id: 1,
                title: "Stub Task".to_string(),
                description: "This is a stub task for testing.".to_string(),
                discussion: vec![],
                stage: Stage::Pending,
                tool: Some(Tool::Stub),
                model: Some(Model::Gpt5Mini),
                parent_task_id: None,
                destination_repo: None,
                destination_branch: None,
                done: false,
                labels: vec![],
                plan: vec![],
            },
        );

        let mut stages = HashMap::new();
        stages.insert(Stage::Pending.to_string(), 1);
        stages.insert(Stage::GoPlanning.to_string(), 2);
        stages.insert(Stage::Planning.to_string(), 3);
        stages.insert(Stage::GoWorking.to_string(), 4);
        stages.insert(Stage::Working.to_string(), 5);

        Self {
            tasks,
            comments: HashMap::new(),
            stages,
            labels: HashSet::new(),
            files: HashMap::new(),
            next_task_id: 2,
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
    async fn get_task(&self, id: u64) -> Result<Task, ZbobrError> {
        let state = self.state.read().unwrap();
        state
            .tasks
            .get(&id)
            .cloned()
            .ok_or_else(|| ZbobrError::Other(format!("Task {id} not found")))
    }

    async fn create_task(
        &self,
        title: &str,
        description: &str,
        stage: Stage,
        tool: Option<Tool>,
        model: Option<Model>,
        parent_task_id: Option<u64>,
        destination_repo: Option<String>,
        destination_branch: Option<String>,
    ) -> Result<u64, ZbobrError> {
        let mut state = self.state.write().unwrap();
        let id = state.next_task_id;
        state.next_task_id += 1;
        let task = Task {
            id,
            title: title.to_string(),
            description: description.to_string(),
            discussion: vec![],
            stage,
            tool,
            model,
            parent_task_id,
            destination_repo,
            destination_branch,
            done: false,
            labels: vec![],
            plan: vec![],
        };
        state.tasks.insert(id, task);
        Ok(id)
    }

    async fn close_task(&self, id: u64) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        state.tasks.remove(&id);
        Ok(())
    }

    async fn get_task_comments(&self, id: u64) -> Result<Vec<String>, ZbobrError> {
        let state = self.state.read().unwrap();
        Ok(state.comments.get(&id).cloned().unwrap_or_default())
    }

    async fn post_task_comment(
        &self,
        id: u64,
        body: &str,
        role: &str,
        hostname: &str,
    ) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        let comments = state.comments.entry(id).or_default();
        comments.push(format!("[{role}@{hostname}] {body}"));
        Ok(())
    }

    async fn set_task_stage(&self, id: u64, stage_name: &str) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        if let Some(task) = state.tasks.get_mut(&id) {
            let stage = match Stage::from_milestone_name(stage_name) {
                Some(s) => s,
                None => return Err(ZbobrError::Other(format!("Unknown stage {stage_name}"))),
            };
            task.stage = stage;
            Ok(())
        } else {
            Err(ZbobrError::Other(format!("Task {id} not found")))
        }
    }

    async fn add_task_label(&self, id: u64, label: Label) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        if let Some(task) = state.tasks.get_mut(&id) {
            if label == Label::Done {
                task.done = true;
            }
            if !task.labels.contains(&label) {
                task.labels.push(label);
            }
            Ok(())
        } else {
            Err(ZbobrError::Other(format!("Task {id} not found")))
        }
    }

    async fn remove_task_label(&self, id: u64, label: Label) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        if let Some(task) = state.tasks.get_mut(&id) {
            if label == Label::Done {
                task.done = false;
            }
            task.labels.retain(|l| l != &label);
            Ok(())
        } else {
            Err(ZbobrError::Other(format!("Task {id} not found")))
        }
    }

    async fn update_task_description(&self, id: u64, description: &str) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        if let Some(task) = state.tasks.get_mut(&id) {
            task.description = description.to_string();
            Ok(())
        } else {
            Err(ZbobrError::Other(format!("Task {id} not found")))
        }
    }

    async fn list_tasks_by_stage(
        &self,
        stage_name: &str,
        tool_filter: Option<Tool>,
    ) -> Result<Vec<Task>, ZbobrError> {
        let state = self.state.read().unwrap();
        let target_stage = match Stage::from_milestone_name(stage_name) {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        let found: Vec<_> = state
            .tasks
            .values()
            .filter(|t| t.stage == target_stage)
            .filter(|t| {
                if let Some(requested_tool) = tool_filter {
                    // Accept tasks that explicitly request this tool OR have no tool specified
                    t.tool == Some(requested_tool) || t.tool.is_none()
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        Ok(found)
    }

    async fn is_task_closed(&self, id: u64) -> Result<bool, ZbobrError> {
        let state = self.state.read().unwrap();
        Ok(!state.tasks.contains_key(&id))
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
        _branch: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        let repo_name = target_repo.split('/').nth(1).unwrap_or(target_repo);
        let task_dir = self.workspace.join(format!("task#{task_id}"));
        let work_dir = task_dir.join(repo_name);

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

    async fn clone_readonly(
        &self,
        target_repo: &str,
        branch: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        self.clone_and_setup(target_repo, branch, task_id).await
    }

    async fn parse_pr_to_repo_branch(&self, pr_ref: &str) -> Result<(String, String), ZbobrError> {
        // Stub implementation: just parse the format and return stub values
        if pr_ref.starts_with("https://github.com/") {
            let parts: Vec<&str> = pr_ref
                .trim_start_matches("https://github.com/")
                .split('/')
                .collect();
            if parts.len() >= 4 && parts[2] == "pull" {
                let repo = format!("{}/{}", parts[0], parts[1]);
                return Ok((repo, "stub-branch".to_string()));
            }
        } else if pr_ref.contains('#') {
            let parts: Vec<&str> = pr_ref.split('#').collect();
            if parts.len() == 2 {
                return Ok((parts[0].to_string(), "stub-branch".to_string()));
            }
        }
        Err(ZbobrError::Other(format!(
            "Invalid PR reference format: {pr_ref}"
        )))
    }

    async fn push_and_create_pr(
        &self,
        _target_repo: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError> {
        Ok(format!("https://github.com/stub/repo/pull/{task_id}"))
    }

    async fn list_stages(&self) -> Result<Vec<(u64, String)>, ZbobrError> {
        let state = self.state.read().unwrap();
        Ok(state.stages.iter().map(|(k, v)| (*v, k.clone())).collect())
    }

    async fn create_stage(&self, title: &str, _description: &str) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        let id = state.stages.len() as u64 + 1;
        state.stages.insert(title.to_string(), id);
        Ok(())
    }

    async fn delete_stage(&self, number: u64) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();
        state.stages.retain(|_, v| *v != number);
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

    async fn setup_repository(&self, _force: bool) -> Result<(), ZbobrError> {
        let mut state = self.state.write().unwrap();

        // Initialize stages
        state.stages.clear();
        state.stages.insert(Stage::Pending.to_string(), 1);
        state.stages.insert(Stage::GoPlanning.to_string(), 2);
        state.stages.insert(Stage::Planning.to_string(), 3);
        state.stages.insert(Stage::GoWorking.to_string(), 4);
        state.stages.insert(Stage::Working.to_string(), 5);

        // Initialize labels
        state.labels.clear();
        state.labels.insert(Label::Done.as_str().to_string());
        state.labels.insert(Label::Question.as_str().to_string());

        // Add tool labels
        for tool in Tool::all() {
            state.labels.insert(format!("tool:{}", tool));
        }

        // Add model labels
        for model in Model::all() {
            state.labels.insert(format!("model:{}", model));
        }

        Ok(())
    }

    async fn validate_connectivity(&self) -> Result<(), ZbobrError> {
        Ok(())
    }

    fn debug_state(&self) -> String {
        let state = self.state.read().unwrap();
        format!("{:?}", state.tasks)
    }
}

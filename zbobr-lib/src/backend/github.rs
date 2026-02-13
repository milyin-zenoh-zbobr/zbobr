use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;

use super::Backend;
use crate::{Model, Parameter, Signal, Stage, Task, Tool, ZbobrConfig, ZbobrError};

pub struct GitHubBackend {
    config: Arc<ZbobrConfig>,
    octocrab: octocrab::Octocrab,
}

impl GitHubBackend {
    pub fn new(config: Arc<ZbobrConfig>, octocrab: octocrab::Octocrab) -> Self {
        Self { config, octocrab }
    }

    /// Convert a Signal to its GitHub label representation.
    fn signal_to_label(signal: Signal) -> String {
        format!("signal:{}", signal.name())
    }

    /// Parse a GitHub label string back to a Signal.
    fn label_to_signal(label: &str) -> Option<Signal> {
        label.strip_prefix("signal:")
            .and_then(|name| name.parse().ok())
    }

    fn is_transient_octocrab_error(error: &octocrab::Error) -> bool {
        match error {
            octocrab::Error::GitHub { source, .. } => source.status_code.is_server_error(),
            _ => true,
        }
    }

    async fn retry_octocrab<T, F, Fut>(&self, op_name: &str, mut f: F) -> Result<T, ZbobrError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, octocrab::Error>>,
    {
        let mut attempt = 0u64;
        loop {
            attempt += 1;
            match f().await {
                Ok(value) => return Ok(value),
                Err(e) => {
                    if attempt < 3 && Self::is_transient_octocrab_error(&e) {
                        tracing::warn!(
                            "Transient GitHub error during {op_name} (attempt {attempt}/3): {e}"
                        );
                        tokio::time::sleep(Duration::from_millis(250 * attempt)).await;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }

    fn parse_repo(&self) -> Result<(&str, &str), ZbobrError> {
        self.config.parse_repo()
    }

    async fn find_stage_number(&self, title: &str) -> Result<Option<u64>, ZbobrError> {
        let stages = self.list_stages().await?;
        Ok(stages.into_iter().find(|(_, t)| t == title).map(|(n, _)| n))
    }

    async fn ensure_fork(&self, target_repo: &str) -> Result<String, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let fork_repo = format!("{}/{}", self.config.fork_owner, repo_name);

        // Check if fork already exists
        let exists = self
            .retry_octocrab("check fork exists", || {
                self.octocrab
                    .get::<RepoResponse, _, _>(format!("/repos/{fork_repo}"), None::<&()>)
            })
            .await
            .is_ok();

        if !exists {
            let parts: Vec<&str> = target_repo.splitn(2, '/').collect();
            if parts.len() != 2 {
                return Err(ZbobrError::Config(format!(
                    "Invalid target repo: {target_repo}"
                )));
            }
            let fork_owner = &self.config.fork_owner;
            let endpoint = format!("/repos/{}/{}/forks", parts[0], parts[1]);
            let payload = serde_json::json!({ "organization": fork_owner });

            tracing::info!("Creating fork of {target_repo} under organization '{fork_owner}' using endpoint {endpoint}", target_repo = target_repo, endpoint = endpoint);
            tracing::debug!("Fork creation payload: {payload}", payload = payload);

            // Create fork under fork_owner (as org)
            self.retry_octocrab("create fork", || {
                self.octocrab.post(&endpoint, Some(&payload))
            })
            .await
            .map_err(|e| {
                    let error_details = format!("{:?}", e);
                    tracing::error!(
                        "Failed to create fork: target_repo={}, fork_owner={}, endpoint={}, error={:?}",
                        target_repo,
                        fork_owner,
                        endpoint,
                        e
                    );
                    ZbobrError::GitHub(
                        format!(
                            "Failed to create fork of {target_repo} under '{fork_owner}': \
                             check if fork_owner is an organization you have access to, \
                             and that your GitHub token has 'repo' and 'admin:org_hook' scopes. \
                             Endpoint: {endpoint}. Error: {e}\n\
                             Debug: {error_details}",
                            target_repo = target_repo,
                            fork_owner = fork_owner,
                            endpoint = endpoint,
                            e = e,
                            error_details = error_details
                        )
                    )
                })
                .map(|_: serde_json::Value| ())?;

            // Wait a moment for the fork to be ready
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        Ok(fork_repo)
    }
}

#[derive(Debug, serde::Deserialize)]
struct IssueResponse {
    number: u64,
    title: String,
    body: Option<String>,
    #[allow(dead_code)]
    state: String,
    milestone: Option<IssueMilestone>,
    labels: Vec<IssueLabel>,
}

#[derive(Debug, serde::Deserialize)]
struct IssueMilestone {
    title: String,
}

#[derive(Debug, serde::Deserialize)]
struct IssueLabel {
    name: String,
}

#[derive(Debug, serde::Deserialize)]
struct CommentResponse {
    user: Option<CommentUser>,
    body: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct CommentUser {
    login: String,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct RepoResponse {
    full_name: String,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct ContentsResponse {
    sha: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct MilestoneResponse {
    number: u64,
    title: String,
}

// Helper methods for GitHubBackend
impl GitHubBackend {
    /// Create or update a file in the task repo via the Contents API.
    /// If sha is provided, updates the existing file; otherwise creates a new one.
    async fn create_or_update_repo_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
        sha: Option<String>,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let encoded = base64_encode(content);

        let url = format!("/repos/{owner}/{repo}/contents/{path}");

        let mut body = serde_json::json!({
            "message": commit_message,
            "content": encoded,
        });

        if let Some(sha) = sha {
            body["sha"] = serde_json::Value::String(sha);
        }

        self.retry_octocrab("create or update repo file", || {
            self.octocrab.put(url.clone(), Some(&body))
        })
        .await
        .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    /// Sync the fork under `self.config.fork_owner` with the given `target_repo` on `branch`.
    /// Uses the server-side merge-upstream endpoint (equivalent to `gh repo sync`).
    /// Returns an error if the operation fails; no fallback is performed.
    pub async fn sync_fork_internal(&self, target_repo: &str, branch: &str) -> Result<(), ZbobrError> {
        // Parse owner/repo
        let parts: Vec<&str> = target_repo.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(ZbobrError::Config(format!("Invalid target repo: {}", target_repo)));
        }
        let upstream_owner = parts[0];

        // Ensure fork exists
        let fork_repo = self.ensure_fork(target_repo).await?; // "fork_owner/repo"

        // Prepare merge-upstream payload
        let endpoint = format!("/repos/{}/merge-upstream", fork_repo);
        let body = serde_json::json!({
            "branch": branch,
            "upstream": format!("{}:{}", upstream_owner, branch),
            "commit_message": format!("Sync fork {} from {}/{}", fork_repo, upstream_owner, branch),
        });

        tracing::info!("Calling merge-upstream for {} -> {}", fork_repo, branch);

        // Call the endpoint and return any error to the caller
        match self.octocrab.post::<serde_json::Value, serde_json::Value>(endpoint, Some(&body)).await {
            Ok(_) => {
                tracing::info!("Successfully synced fork {} from {}/{}", fork_repo, upstream_owner, branch);
                Ok(())
            }
            Err(e) => {
                tracing::error!("merge-upstream failed for {}: {}", fork_repo, e);
                return Err(e.into());
            }
        }
    }

    /// Update a label's color and description.
    async fn update_label(
        &self,
        name: &str,
        color: &str,
        description: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let url = format!("/repos/{owner}/{repo}/labels/{name}");
        let body = serde_json::json!({
            "color": color,
            "description": description,
        });
        self.retry_octocrab("update label", || {
            self.octocrab.patch(url.clone(), Some(&body))
        })
        .await
        .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    /// Create and push a placeholder file for a branch to ensure it has at least one commit.
    /// This is pushed immediately to the fork.
    async fn create_and_push_placeholder(
        &self,
        work_dir: &std::path::Path,
        branch_name: &str,
    ) -> Result<(), ZbobrError> {
        // Create placeholder file and commit it
        crate::backend::create_placeholder_commit(
            work_dir,
            branch_name,
            &self.config.git_user_name,
            &self.config.git_user_email,
        )
        .await?;

        // Push to fork immediately with tracking
        let push_status = tokio::process::Command::new("git")
            .args(["push", "-u", "fork", "HEAD"])
            .current_dir(work_dir)
            .status()
            .await
            .map_err(|e| ZbobrError::Other(format!("Failed to run git push: {}", e)))?;

        if !push_status.success() {
            return Err(ZbobrError::Other("git push for placeholder failed".to_string()));
        }

        tracing::info!("Successfully created and pushed placeholder for branch {}", branch_name);
        Ok(())
    }
}

#[async_trait]
impl Backend for GitHubBackend {
    async fn get_task(&self, id: u64) -> Result<Task, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let issue: IssueResponse = self
            .retry_octocrab("get issue", || {
                self.octocrab
                    .get(format!("/repos/{owner}/{repo}/issues/{id}"), None::<&()>)
            })
            .await?;

        let stage = match issue.milestone.as_ref().map(|m| m.title.as_str()) {
            Some(t) => Stage::from_milestone_name(t).unwrap_or(Stage::Planning),
            _ => Stage::Planning,
        };

        let body = issue.body.unwrap_or_default();
        // Parse full description including PARAMETERS, PLAN and CHECKLIST
        let (description, params_map, plan, checklist) = super::parse_description_full(&body);

        let tool = issue.labels.iter().find_map(|l| {
            if let Some(name) = l.name.strip_prefix("tool:") {
                match name {
                    "copilot" => Some(Tool::Copilot),
                    "claude" => Some(Tool::Claude),
                    "stub" => Some(Tool::Stub),
                    _ => None,
                }
            } else {
                None
            }
        });

        let model = issue.labels.iter().find_map(|l| {
            if let Some(name) = l.name.strip_prefix("model:") {
                name.parse::<Model>().ok()
            } else {
                None
            }
        });

        // Build parameters HashMap<Parameter,String> from parsed parameters
        let mut parameters = HashMap::new();
        if let Some(repo) = params_map.get(Parameter::DestinationRepository.name()) {
            parameters.insert(Parameter::DestinationRepository, repo.clone());
        }
        if let Some(branch) = params_map.get(Parameter::DestinationBranch.name()) {
            parameters.insert(Parameter::DestinationBranch, branch.clone());
        }
        if let Some(branch) = params_map.get(Parameter::WorkBranch.name()) {
            parameters.insert(Parameter::WorkBranch, branch.clone());
        }
        if let Some(url) = params_map.get(Parameter::PrUrl.name()) {
            parameters.insert(Parameter::PrUrl, url.clone());
        }

        // Check if 'done' signal is present
        let done = issue
            .labels
            .iter()
            .any(|l| Self::label_to_signal(&l.name) == Some(Signal::Done));

        // Extract signal from labels (highest priority wins)
        let signal = issue
            .labels
            .iter()
            .filter_map(|l| Self::label_to_signal(&l.name))
            .min(); // min() because lower enum value = higher priority

        // description, plan and checklist were already extracted above

        // Discussion is not fetched by default for performance in listings,
        // but for a single get_task we could.
        // However, the trait has get_task_comments for that.
        // I'll populate it with empty for now.
        Ok(Task {
            id: issue.number,
            title: issue.title,
            description,
            plan,
            discussion: vec![],
            stage,
            tool,
            model,
            parameters,
            done,
            checklist,
            signal,
            etag: Some(body), // Store the full body as version info for conflict detection
        })
    }

    async fn create_task(
        &self,
        title: &str,
        description: &str,
        stage: Stage,
        tool: Option<Tool>,
        model: Option<Model>,
        parameters: HashMap<Parameter, String>,
    ) -> Result<u64, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        // Serialize parameters into the PARAMETERS section inside the issue body
        let mut params_text: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        if let Some(v) = parameters.get(&Parameter::DestinationRepository) {
            params_text.insert(Parameter::DestinationRepository.name().to_string(), v.clone());
        }
        if let Some(v) = parameters.get(&Parameter::DestinationBranch) {
            params_text.insert(Parameter::DestinationBranch.name().to_string(), v.clone());
        }
        if let Some(v) = parameters.get(&Parameter::WorkBranch) {
            params_text.insert(Parameter::WorkBranch.name().to_string(), v.clone());
        }
        if let Some(v) = parameters.get(&Parameter::PrUrl) {
            params_text.insert(Parameter::PrUrl.name().to_string(), v.clone());
        }
        let body = crate::backend::serialize_description_full(description, &params_text, "", &[]);

        let stage_number = self.find_stage_number(stage.milestone_name()).await?;

        let mut labels = vec![];
        if let Some(t) = tool {
            labels.push(format!("tool:{}", t));
        }
        if let Some(m) = model {
            labels.push(format!("model:{}", m));
        }

        let issue = self
            .retry_octocrab("create issue", || async {
                let issues = self.octocrab.issues(owner, repo);
                let mut builder = issues.create(title).body(body.clone());

                if let Some(n) = stage_number {
                    builder = builder.milestone(n);
                }

                if !labels.is_empty() {
                    builder = builder.labels(labels.clone());
                }

                builder.send().await
            })
            .await?;
        Ok(issue.number)
    }

    async fn close_task(&self, id: u64) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let url = format!("/repos/{owner}/{repo}/issues/{id}");
        let body = serde_json::json!({ "state": "closed" });
        self.retry_octocrab("close issue", || {
            self.octocrab.patch(url.clone(), Some(&body))
        })
        .await
        .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    async fn get_task_comments(&self, id: u64) -> Result<Vec<String>, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let comments: Vec<CommentResponse> = self
            .retry_octocrab("list issue comments", || {
                self.octocrab.get(
                    format!("/repos/{owner}/{repo}/issues/{id}/comments"),
                    None::<&()>,
                )
            })
            .await?;

        Ok(comments
            .into_iter()
            .map(|c| {
                let user = c.user.map(|u| u.login).unwrap_or_else(|| "unknown".into());
                let body = c.body.unwrap_or_default();
                format!("{user}: {body}")
            })
            .collect())
    }

    async fn post_task_comment(
        &self,
        id: u64,
        body: &str,
        role: &str,
        hostname: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let formatted_body = format!("**[{role}@{hostname}]**\n\n{body}");
        self.retry_octocrab("create issue comment", || async {
            self.octocrab
                .issues(owner, repo)
                .create_comment(id, &formatted_body)
                .await
        })
        .await?;
        Ok(())
    }

    async fn set_task_stage(&self, id: u64, stage_name: &str) -> Result<(), ZbobrError> {
        let stage_number = self
            .find_stage_number(stage_name)
            .await?
            .ok_or_else(|| ZbobrError::GitHub(format!("Milestone '{stage_name}' not found")))?;

        let (owner, repo) = self.parse_repo()?;
        let url = format!("/repos/{owner}/{repo}/issues/{id}");
        let body = serde_json::json!({ "milestone": stage_number });
        self.retry_octocrab("set issue milestone", || {
            self.octocrab.patch(url.clone(), Some(&body))
        })
        .await
        .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    async fn set_task_signal(&self, id: u64, signal: Option<Signal>) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        
        // Remove all existing signal labels
        for sig in Signal::all() {
            let label = Self::signal_to_label(*sig);
            let _ = self
                .retry_octocrab("remove signal label", || async {
                    self.octocrab
                        .issues(owner, repo)
                        .remove_label(id, &label)
                        .await
                })
                .await;
        }
        
        // Add new signal label if provided
        if let Some(sig) = signal {
            let label = Self::signal_to_label(sig);
            let labels: Vec<String> = vec![label];
            self.retry_octocrab("add signal label", || async {
                self.octocrab
                    .issues(owner, repo)
                    .add_labels(id, &labels)
                    .await
            })
            .await?;
        }
        
        Ok(())
    }

    async fn update_task_description(&self, id: u64, description: &str) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        // Just store the description as-is, it should be pre-formatted by the caller
        let url = format!("/repos/{owner}/{repo}/issues/{id}");
        let body = serde_json::json!({ "body": description });
        self.retry_octocrab("update issue body", || {
            self.octocrab.patch(url.clone(), Some(&body))
        })
        .await
        .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    async fn update_task_description_with_conflict_detection(
        &self,
        id: u64,
        expected_description: &str,
        new_description: &str,
    ) -> Result<(), ZbobrError> {
        const MAX_RETRIES: u32 = 3;
        let mut retry_count = 0;
        let current_expected = expected_description;

        loop {
            retry_count += 1;
            
            // Try to update
            let (owner, repo) = self.parse_repo()?;
            let url = format!("/repos/{owner}/{repo}/issues/{id}");
            let body = serde_json::json!({ "body": new_description });
            
            match self.retry_octocrab("update issue body", || {
                self.octocrab.patch::<serde_json::Value, _, _>(url.clone(), Some(&body))
            }).await {
                Ok(_) => return Ok(()), // Success
                Err(_) if retry_count >= MAX_RETRIES => return Ok(()), // Give up after max retries
                Err(_) => {} // Continue to conflict detection
            }

            // Re-read the current state
            let current_issue: IssueResponse = self
                .retry_octocrab("get issue for conflict detection", || {
                    self.octocrab
                        .get(format!("/repos/{owner}/{repo}/issues/{id}"), None::<&()>)
                })
                .await?;

            let current_body = current_issue.body.unwrap_or_default();

            // Check if there was a concurrent modification
            if current_body != current_expected {
                // Conflict detected! Merge the changes
                let merged = super::merge_concurrent_description_updates(
                    &current_expected,
                    &current_body,
                    new_description,
                );
                
                // Write the merged description
                let url = format!("/repos/{owner}/{repo}/issues/{id}");
                let merge_body = serde_json::json!({ "body": &merged });
                self.retry_octocrab("update issue body with merged content", || {
                    self.octocrab.patch::<serde_json::Value, _, _>(url.clone(), Some(&merge_body))
                })
                .await
                .map(|_: serde_json::Value| ())?;
                return Ok(());
            } else {
                // No conflict detected, but write failed and we're not out of retries
                // Just try again
                if retry_count < MAX_RETRIES {
                    continue;
                } else {
                    return Ok(());
                }
            }
        }
    }

    async fn list_tasks_by_stage(
        &self,
        stage_name: &str,
        tool: Option<Tool>,
    ) -> Result<Vec<Task>, ZbobrError> {
        let stage_number = match self.find_stage_number(stage_name).await? {
            Some(n) => n,
            None => return Ok(vec![]),
        };

        let (owner, repo) = self.parse_repo()?;
        let params = vec![
            ("milestone", stage_number.to_string()),
            ("state", "open".to_string()),
        ];

        // Fetch all issues for this milestone, don't filter by tool label at API level
        let issues: Vec<IssueResponse> = self
            .retry_octocrab("list issues", || {
                self.octocrab
                    .get(format!("/repos/{owner}/{repo}/issues"), Some(&params))
            })
            .await?;

        let mut tasks = Vec::new();
        for issue in issues {
            let stage = match issue.milestone.as_ref().map(|m| m.title.as_str()) {
                Some(t) => Stage::from_milestone_name(t).unwrap_or(Stage::Planning),
                _ => Stage::Planning,
            };

            let body = issue.body.unwrap_or_default();
            // Parse full description including PARAMETERS, PLAN and CHECKLIST
            let (description, params_map, plan, checklist) = super::parse_description_full(&body);
            let task_tool = issue.labels.iter().find_map(|l| {
                if let Some(name) = l.name.strip_prefix("tool:") {
                    match name {
                        "copilot" => Some(Tool::Copilot),
                        "claude" => Some(Tool::Claude),
                        "stub" => Some(Tool::Stub),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            // Filter client-side: if tool filter is provided, only include tasks that:
            // - have no tool label (can be taken by anyone), OR
            // - have a matching tool label
            if let Some(filter_tool) = tool
                && let Some(t) = task_tool
                && t != filter_tool
            {
                continue; // Skip tasks with different tool label
            }
            // If task_tool is None, include it (no label = any bot can take it)

            let model = issue.labels.iter().find_map(|l| {
                if let Some(name) = l.name.strip_prefix("model:") {
                    name.parse::<Model>().ok()
                } else {
                    None
                }
            });

            // Build parameters HashMap<Parameter,String> from parsed parameters
            let mut parameters = HashMap::new();
            if let Some(repo) = params_map.get(Parameter::DestinationRepository.name()) {
                parameters.insert(Parameter::DestinationRepository, repo.clone());
            }
            if let Some(branch) = params_map.get(Parameter::DestinationBranch.name()) {
                parameters.insert(Parameter::DestinationBranch, branch.clone());
            }
            if let Some(branch) = params_map.get(Parameter::WorkBranch.name()) {
                parameters.insert(Parameter::WorkBranch, branch.clone());
            }
            if let Some(url) = params_map.get(Parameter::PrUrl.name()) {
                parameters.insert(Parameter::PrUrl, url.clone());
            }

            // Check if 'done' signal is present
            let done = issue
                .labels
                .iter()
                .any(|l| Self::label_to_signal(&l.name) == Some(Signal::Done));

            // Extract signal from labels (highest priority wins)
            let signal = issue
                .labels
                .iter()
                .filter_map(|l| Self::label_to_signal(&l.name))
                .min(); // min() because lower enum value = higher priority

            // description, plan and checklist were already extracted above

            tasks.push(Task {
                id: issue.number,
                title: issue.title,
                description,
                plan,
                discussion: vec![],
                stage,
                tool: task_tool,
                model,
                parameters,
                done,
                checklist,
                signal,
                etag: Some(body), // Store the full body as version info for conflict detection
            });
        }
        Ok(tasks)
    }

    async fn is_task_closed(&self, id: u64) -> Result<bool, ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let issue: IssueResponse = self
            .retry_octocrab("get issue state", || {
                self.octocrab
                    .get(format!("/repos/{owner}/{repo}/issues/{id}"), None::<&()>)
            })
            .await?;
        Ok(issue.state == "closed")
    }

    async fn repo_file_exists(&self, path: &str) -> Result<bool, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let result = self
            .retry_octocrab("check repo file exists", || {
                self.octocrab.get::<ContentsResponse, _, _>(
                    format!("/repos/{owner}/{repo}/contents/{path}"),
                    None::<&()>,
                )
            })
            .await;
        Ok(result.is_ok())
    }

    async fn create_repo_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<(), ZbobrError> {
        self.create_or_update_repo_file(path, content, commit_message, None)
            .await
    }

    async fn ensure_task_repo_exists(&self) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let exists = self
            .retry_octocrab("check task repo exists", || {
                self.octocrab
                    .get::<RepoResponse, _, _>(format!("/repos/{owner}/{repo}"), None::<&()>)
            })
            .await
            .is_ok();

        if !exists {
            tracing::info!("Task repo {owner}/{repo} does not exist, creating...");
            // Try creating as org repo first, fall back to user repo
            let org_url = format!("/orgs/{owner}/repos");
            let org_body = serde_json::json!({
                "name": repo,
                "private": true,
                "auto_init": false,
            });
            let result = self
                .retry_octocrab("create org repo", || async {
                    self.octocrab.post(org_url.clone(), Some(&org_body)).await
                })
                .await;

            match result {
                Ok(_v) => {
                    let _: serde_json::Value = _v;
                    tracing::info!("Created private org repo {owner}/{repo}");
                }
                Err(_) => {
                    // Fall back to user repo
                    let user_url = "/user/repos".to_string();
                    let user_body = serde_json::json!({
                        "name": repo,
                        "private": true,
                        "auto_init": false,
                    });
                    self.retry_octocrab("create user repo", || async {
                        self.octocrab.post(user_url.clone(), Some(&user_body)).await
                    })
                    .await
                    .map(|_: serde_json::Value| ())?;
                    tracing::info!("Created private user repo {owner}/{repo}");
                }
            }
            // Wait for repo init
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        Ok(())
    }

    async fn clone_and_setup(
        &self,
        target_repo: &str,
        branch: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let task_dir = self.config.workspace.join(format!("task#{task_id}"));
        let work_dir = task_dir.join(repo_name);

        tokio::fs::create_dir_all(&task_dir).await?;

        // Clone if not already present
        if !work_dir.exists() {
            tracing::info!("Cloning {target_repo} into {}", work_dir.display());
            // Clone only the specified branch to keep the agent isolated from other branches.
            let status = tokio::process::Command::new("gh")
                .args([
                    "repo",
                    "clone",
                    target_repo,
                    work_dir.to_str().unwrap(),
                    "--",
                    "--branch",
                    branch,
                    "--single-branch",
                    "--depth",
                    "1",
                ])
                .env("GH_TOKEN", &self.config.owner_github_token)
                .env("GITHUB_TOKEN", &self.config.owner_github_token)
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other(format!("Failed to clone {target_repo}")));
            }
        } else {
            // Fetch latest changes from origin if repo already exists
            tracing::info!("Updating {target_repo} in {}", work_dir.display());
            // Fetch only the destination branch to avoid exposing other branches to the agent.
            let fetch_status = tokio::process::Command::new("git")
                .args(["fetch", "--depth", "1", "origin", branch])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !fetch_status.success() {
                tracing::warn!(
                    "Failed to fetch latest changes for {target_repo}, using existing state"
                );
            }
        }

        // Checkout the requested branch
        tracing::info!("Checking out branch {branch}");
        let checkout_status = tokio::process::Command::new("git")
            .args(["checkout", branch])
            .current_dir(&work_dir)
            .status()
            .await?;
        if !checkout_status.success() {
            // Try to checkout from origin if local branch doesn't exist
            let checkout_remote_status = tokio::process::Command::new("git")
                .args(["checkout", "-b", branch, &format!("origin/{branch}")])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !checkout_remote_status.success() {
                return Err(ZbobrError::Other(format!(
                    "Failed to checkout branch {branch}"
                )));
            }
        }

        // Ensure fork exists
        let fork_repo = self.ensure_fork(target_repo).await?;

        // Add fork remote if not present
        let remote_check = tokio::process::Command::new("git")
            .args(["remote", "get-url", "fork"])
            .current_dir(&work_dir)
            .output()
            .await?;

        if !remote_check.status.success() {
            tracing::info!("Adding fork remote for {fork_repo}");
            let status = tokio::process::Command::new("git")
                .args([
                    "remote",
                    "add",
                    "fork",
                    &format!("https://github.com/{fork_repo}.git"),
                ])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other("Failed to add fork remote".into()));
            }
        }

        // Create feature branch from the requested branch
        let branch_name = format!("fix{task_id}/implementation");
        let branch_exists = tokio::process::Command::new("git")
            .args(["rev-parse", "--verify", &branch_name])
            .current_dir(&work_dir)
            .output()
            .await?;

        if branch_exists.status.success() {
            let _ = tokio::process::Command::new("git")
                .args(["checkout", &branch_name])
                .current_dir(&work_dir)
                .status()
                .await?;
        } else {
            let status = tokio::process::Command::new("git")
                .args(["checkout", "-b", &branch_name])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other(format!(
                    "Failed to create branch {branch_name}"
                )));
            }
            // Create a zero-sized placeholder file inside .zbobr to ensure
            // the branch has at least one commit (some upstream PR APIs
            // reject branches with no commits). The file is named after
            // the branch. It will be removed later when actual changes
            // are committed and pushed.
            // Create and immediately push the placeholder to ensure it's on the fork
            self.create_and_push_placeholder(&work_dir, &branch_name).await?;
        }

        Ok(work_dir)
    }

    async fn clone_readonly(
        &self,
        target_repo: &str,
        branch: &str,
        task_id: u64,
    ) -> Result<PathBuf, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let task_dir = self.config.workspace.join(format!("task#{task_id}"));
        let work_dir = task_dir.join(repo_name);

        tokio::fs::create_dir_all(&task_dir).await?;

        if !work_dir.exists() {
            tracing::info!(
                "Cloning {target_repo} (read-only) into {}",
                work_dir.display()
            );
            let status = tokio::process::Command::new("gh")
                .args([
                    "repo",
                    "clone",
                    target_repo,
                    work_dir.to_str().unwrap(),
                    "--",
                    "--branch",
                    branch,
                    "--single-branch",
                    "--depth",
                    "1",
                ])
                .env("GH_TOKEN", &self.config.owner_github_token)
                .env("GITHUB_TOKEN", &self.config.owner_github_token)
                .status()
                .await?;
            if !status.success() {
                return Err(ZbobrError::Other(format!("Failed to clone {target_repo}")));
            }
        } else {
            // Fetch latest changes from origin if repo already exists
            tracing::info!(
                "Updating {target_repo} (read-only) in {}",
                work_dir.display()
            );

            let fetch_status = tokio::process::Command::new("git")
                .args(["fetch", "--depth", "1", "origin", branch])
                .current_dir(&work_dir)
                .status()
                .await?;

            if !fetch_status.success() {
                tracing::warn!(
                    "Failed to fetch latest changes for {target_repo}, using existing state"
                );
            }
        }

        // Checkout the requested branch
        tracing::info!("Checking out branch {branch} (read-only)");
        let checkout_status = tokio::process::Command::new("git")
            .args(["checkout", branch])
            .current_dir(&work_dir)
            .status()
            .await?;
        if !checkout_status.success() {
            // Try to checkout from origin if local branch doesn't exist
            let checkout_remote_status = tokio::process::Command::new("git")
                .args(["checkout", "-b", branch, &format!("origin/{branch}")])
                .current_dir(&work_dir)
                .status()
                .await?;
            if !checkout_remote_status.success() {
                return Err(ZbobrError::Other(format!(
                    "Failed to checkout branch {branch}"
                )));
            }
        }

        Ok(work_dir)
    }

    async fn push_and_create_pr(
        &self,
        target_repo: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError> {
        let repo_name = target_repo
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Config(format!("Invalid repo format: {target_repo}")))?;

        let work_dir = self
            .config
            .workspace
            .join(format!("task#{task_id}"))
            .join(repo_name);

        if !work_dir.exists() {
            return Err(ZbobrError::Other(format!(
                "Work directory does not exist: {}",
                work_dir.display()
            )));
        }

        let branch_name = format!("fix{task_id}/implementation");

        // Push to fork
        // If a placeholder file exists for this branch and there are local
        // changes to commit, remove the placeholder and commit its removal
        // together with the other changes so the resulting PR contains the
        // real work only.
        let zbobr_placeholder = work_dir.join(".zbobr").join(&branch_name);
        if zbobr_placeholder.exists() {
            // Check for local uncommitted changes
            match tokio::process::Command::new("git")
                .args(["status", "--porcelain"]) 
                .current_dir(&work_dir)
                .output()
                .await
            {
                Ok(out) => {
                    if !out.stdout.is_empty() {
                        tracing::info!("Local changes detected; removing placeholder {} and committing changes", branch_name);
                        let _ = tokio::process::Command::new("git")
                            .args(["rm", "-f", format!(".zbobr/{}", &branch_name).as_str()])
                            .current_dir(&work_dir)
                            .status()
                            .await;
                        let _ = tokio::process::Command::new("git")
                            .args(["add", "-A"]) 
                            .current_dir(&work_dir)
                            .status()
                            .await;
                        let commit_msg = format!("chore: remove placeholder {} and apply changes", &branch_name);
                        let commit_status = tokio::process::Command::new("git")
                            .args(["commit", "-m", &commit_msg])
                            .current_dir(&work_dir)
                            .status()
                            .await;
                        if let Err(e) = commit_status {
                            tracing::warn!("Failed to commit after removing placeholder: {}", e);
                        }
                    }
                }
                Err(e) => tracing::warn!("Failed to check git status: {}", e),
            }
        }

        tracing::info!("Pushing {branch_name} to fork");
        let status = tokio::process::Command::new("git")
            .args(["push", "fork", "HEAD"])
            .current_dir(&work_dir)
            .status()
            .await?;
        if !status.success() {
            return Err(ZbobrError::Other("Failed to push to fork".into()));
        }

        // Create PR using octocrab API, targeting the destination repository with head from the fork
        let task = self.get_task(task_id).await?;
        let pr_title = format!("Fix #{task_id}: {}", task.title);
        let pr_body = format!("Resolves #{task_id}\n\nImplementation for: {}", task.title);

        let pr_payload = serde_json::json!({
            "title": pr_title,
            "head": format!("{}:{branch_name}", self.config.fork_owner),
            "body": pr_body,
        });

        #[derive(serde::Deserialize)]
        struct PrResponse {
            html_url: String,
        }

        let pr_endpoint = format!("/repos/{target_repo}/pulls");
        let response: PrResponse = self
            .octocrab
            .post(pr_endpoint, Some(&pr_payload))
            .await
            .map_err(|e| ZbobrError::GitHub(e.to_string()))?;

        Ok(response.html_url)
    }

    async fn create_pr_in_fork(
        &self,
        destination_repository: &str,
        work_branch: &str,
        destination_branch: &str,
        task_id: u64,
    ) -> Result<String, ZbobrError> {
        let repo_name = destination_repository
            .split('/')
            .nth(1)
            .ok_or_else(|| ZbobrError::Other("Invalid destination_repository format".to_string()))?;

        let fork_repo = format!("{}/{}", self.config.fork_owner, repo_name);

        // Create PR within the fork using octocrab GitHub API
        let task = self.get_task(task_id).await?;
        let pr_title = format!("Fix #{}: {}", task_id, task.title);
        let pr_body = format!(
            "Resolves #{}\n\nImplementation for: {}",
            task_id, task.title
        );

        tracing::info!(
            "Creating PR in {} from {} to {} using octocrab",
            fork_repo,
            work_branch,
            destination_branch
        );

        let pr_payload = serde_json::json!({
            "title": pr_title,
            "head": work_branch,
            "base": destination_branch,
            "body": pr_body,
        });

        let pr_endpoint = format!("/repos/{fork_repo}/pulls");

        #[derive(serde::Deserialize)]
        struct PrResponse {
            html_url: String,
        }

        let response: PrResponse = self
            .retry_octocrab("create PR", || {
                self.octocrab.post(pr_endpoint.clone(), Some(&pr_payload))
            })
            .await?;

        Ok(response.html_url)
    }

    async fn sync_fork(&self, target_repo: &str, branch: &str) -> Result<(), ZbobrError> {
        // Delegate to internal implementation
        self.sync_fork_internal(target_repo, branch).await
    }

    async fn parse_pr_to_repo_branch(&self, pr_ref: &str) -> Result<(String, String), ZbobrError> {
        let (owner, repo, pr_number) = if pr_ref.starts_with("https://github.com/") {
            // Parse URL format: https://github.com/owner/repo/pull/123
            let parts: Vec<&str> = pr_ref
                .trim_start_matches("https://github.com/")
                .split('/')
                .collect();
            if parts.len() >= 4 && parts[2] == "pull" {
                let owner = parts[0];
                let repo = parts[1];
                let pr_num = parts[3].parse::<u64>().map_err(|_| {
                    ZbobrError::Other(format!("Invalid PR number in URL: {pr_ref}"))
                })?;
                (owner.to_string(), repo.to_string(), pr_num)
            } else {
                return Err(ZbobrError::Other(format!(
                    "Invalid PR URL format: {pr_ref}"
                )));
            }
        } else if pr_ref.contains('#') {
            // Parse short format: owner/repo#123
            let parts: Vec<&str> = pr_ref.split('#').collect();
            if parts.len() == 2 {
                let repo_parts: Vec<&str> = parts[0].split('/').collect();
                if repo_parts.len() == 2 {
                    let owner = repo_parts[0];
                    let repo = repo_parts[1];
                    let pr_num = parts[1].parse::<u64>().map_err(|_| {
                        ZbobrError::Other(format!("Invalid PR number: {}", parts[1]))
                    })?;
                    (owner.to_string(), repo.to_string(), pr_num)
                } else {
                    return Err(ZbobrError::Other(format!(
                        "Invalid repo format in PR reference: {pr_ref}"
                    )));
                }
            } else {
                return Err(ZbobrError::Other(format!(
                    "Invalid PR reference format: {pr_ref}"
                )));
            }
        } else {
            return Err(ZbobrError::Other(format!(
                "PR reference must be a URL or owner/repo#number format: {pr_ref}"
            )));
        };

        // Use octocrab API to get PR details (head ref)
        #[derive(serde::Deserialize)]
        struct PrHead {
            #[serde(rename = "ref")]
            ref_name: String,
        }
        #[derive(serde::Deserialize)]
        struct PrView {
            head: PrHead,
        }

        let pr_endpoint = format!("/repos/{owner}/{repo}/pulls/{pr_number}");
        let pr: PrView = self
            .octocrab
            .get(pr_endpoint, None::<&()>)
            .await
            .map_err(|e| ZbobrError::GitHub(e.to_string()))?;

        let branch = pr.head.ref_name;
        let repo_full = format!("{owner}/{repo}");

        Ok((repo_full, branch))
    }

    async fn list_stages(&self) -> Result<Vec<(u64, String)>, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let milestones: Vec<MilestoneResponse> = self
            .retry_octocrab("list milestones", || {
                self.octocrab
                    .get(format!("/repos/{owner}/{repo}/milestones"), None::<&()>)
            })
            .await?;
        Ok(milestones
            .into_iter()
            .map(|m| (m.number, m.title))
            .collect())
    }

    async fn create_stage(&self, title: &str, description: &str) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let url = format!("/repos/{owner}/{repo}/milestones");
        let body = serde_json::json!({
            "title": title,
            "description": description,
            "state": "open"
        });
        self.retry_octocrab("create milestone", || {
            self.octocrab.post(url.clone(), Some(&body))
        })
        .await
        .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    async fn delete_stage(&self, number: u64) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let url = format!("/repos/{owner}/{repo}/milestones/{number}");
        let _response = self
            .retry_octocrab("delete milestone", || {
                self.octocrab._delete(url.clone(), None::<&()>)
            })
            .await?;
        Ok(())
    }

    async fn list_labels(&self) -> Result<Vec<String>, ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        let labels: Vec<octocrab::models::Label> = self
            .retry_octocrab("list labels", || async {
                self.octocrab
                    .issues(owner, repo)
                    .list_labels_for_repo()
                    .per_page(100)
                    .send()
                    .await
            })
            .await?
            .items;
        Ok(labels.into_iter().map(|l| l.name).collect())
    }

    async fn create_label(
        &self,
        name: &str,
        color: &str,
        description: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.parse_repo()?;
        self.retry_octocrab("create label", || async {
            self.octocrab
                .issues(owner, repo)
                .create_label(name, color, description)
                .await
        })
        .await?;
        Ok(())
    }

    async fn setup_repository(&self, force: bool) -> Result<(), ZbobrError> {
        tracing::info!(
            "Setting up GitHub repo: {} (force: {})",
            self.config.task_repo,
            force
        );

        // Ensure the task repo exists
        self.ensure_task_repo_exists().await?;

        // Create stages
        let desired_stages = [
            Stage::Pending,
            Stage::GoPlanning,
            Stage::Planning,
            Stage::GoWorking,
            Stage::Working,
            Stage::GoReviewing,
            Stage::Reviewing,
        ];
        let existing = self.list_stages().await?;
        let existing_titles: Vec<&str> = existing.iter().map(|(_, t)| t.as_str()).collect();

        for stage in &desired_stages {
            let title = stage.milestone_name();
            if existing_titles.contains(&title) {
                tracing::info!("Stage '{title}' already exists");
            } else {
                tracing::info!("Creating stage '{title}'");
                self.create_stage(title, stage_description(*stage)).await?;
            }
        }

        // Delete extra stages
        let desired_titles: Vec<&str> = desired_stages.iter().map(|s| s.milestone_name()).collect();
        for (number, title) in &existing {
            if !desired_titles.contains(&title.as_str()) {
                tracing::info!("Deleting stage '{title}'");
                self.delete_stage(*number).await?;
            }
        }

        // Create labels
        let existing_labels = self.list_labels().await?;

        const SIGNAL_LABEL_COLOR: &str = "5319e7";
        const TOOL_LABEL_COLOR: &str = "d4c5f9";
        const MODEL_LABEL_COLOR: &str = "bfd4f2";

        // Create signal labels for all available signals
        for signal in Signal::all() {
            let signal_label = Self::signal_to_label(*signal);
            let signal_desc = format!("Signal: {}", signal.name());
            if !existing_labels.contains(&signal_label) {
                tracing::info!("Creating label '{signal_label}'");
                self.create_label(&signal_label, SIGNAL_LABEL_COLOR, &signal_desc)
                    .await?;
            } else if force {
                tracing::info!("Updating label '{signal_label}' (force)");
                self.update_label(&signal_label, SIGNAL_LABEL_COLOR, &signal_desc)
                    .await?;
            } else {
                tracing::info!("Label '{signal_label}' already exists");
            }
        }

        // Create tool labels for all available tools
        for tool in Tool::all() {
            let tool_label = format!("tool:{}", tool);
            let tool_desc = format!("Use {} tool", tool);
            if !existing_labels.contains(&tool_label) {
                tracing::info!("Creating label '{tool_label}'");
                self.create_label(&tool_label, TOOL_LABEL_COLOR, &tool_desc)
                    .await?;
            } else if force {
                tracing::info!("Updating label '{tool_label}' (force)");
                self.update_label(&tool_label, TOOL_LABEL_COLOR, &tool_desc)
                    .await?;
            } else {
                tracing::info!("Label '{tool_label}' already exists");
            }
        }

        // Create model labels for all available models
        for model in Model::all() {
            let model_label = format!("model:{}", model);
            let model_desc = format!("Use {} model", model);
            if !existing_labels.contains(&model_label) {
                tracing::info!("Creating label '{model_label}'");
                self.create_label(&model_label, MODEL_LABEL_COLOR, &model_desc)
                    .await?;
            } else if force {
                tracing::info!("Updating label '{model_label}' (force)");
                self.update_label(&model_label, MODEL_LABEL_COLOR, &model_desc)
                    .await?;
            } else {
                tracing::info!("Label '{model_label}' already exists");
            }
        }

        tracing::info!("GitHub setup complete for {}", self.config.task_repo);
        Ok(())
    }

    async fn validate_connectivity(&self) -> Result<(), ZbobrError> {
        let fork_owner = &self.config.fork_owner;
        let fork_owner_exists = self
            .retry_octocrab("check fork owner", || {
                self.octocrab
                    .get::<serde_json::Value, _, _>(format!("/users/{fork_owner}"), None::<&()>)
            })
            .await
            .is_ok();
        if !fork_owner_exists {
            return Err(ZbobrError::Config(format!(
                "fork_owner '{fork_owner}' does not exist on GitHub as a user or organization.\n  \
                 Check your fork_owner setting and ensure the account exists."
            )));
        }

        let (owner, repo) = self.parse_repo()?;
        let task_repo_exists = self
            .retry_octocrab("check task repo", || {
                self.octocrab
                    .get::<RepoResponse, _, _>(format!("/repos/{owner}/{repo}"), None::<&()>)
            })
            .await
            .is_ok();
        if !task_repo_exists {
            return Err(ZbobrError::Config(format!(
                "task_repo '{owner}/{repo}' is not accessible on GitHub.\n  \
                 Check your task_repo setting and ensure the repository exists \
                 and your token has access to it."
            )));
        }

        Ok(())
    }

    fn debug_state(&self) -> String {
        "GitHubBackend".to_string()
    }
}

/// Stage descriptions.
fn stage_description(stage: Stage) -> &'static str {
    match stage {
        Stage::Pending => "Task is under user's control, bot ignores it",
        Stage::GoPlanning => "Task must be taken by planner agent, any matching bot can take it",
        Stage::Planning => "Task is in planning, other bots ignore it",
        Stage::GoWorking => "Task must be taken by worker agent, any matching bot can take it",
        Stage::Working => "Task is in work, other bots ignore it",
        Stage::GoReviewing => "Task must be taken by reviewer agent, any matching bot can take it",
        Stage::Reviewing => "Task is in review, other bots ignore it",
    }
}

/// Simple base64 encoder (standard alphabet, with padding).
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

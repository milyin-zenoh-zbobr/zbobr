use crate::{Stage, Task, Zbobr, ZbobrError};

#[derive(Debug, serde::Deserialize)]
struct IssueResponse {
    number: u64,
    title: String,
    body: Option<String>,
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

impl Zbobr {
    /// Get an issue as a Task.
    pub(crate) async fn get_issue(&self, issue_number: u64) -> Result<Task, ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let issue: IssueResponse = self
            .octocrab
            .get(
                format!("/repos/{owner}/{repo}/issues/{issue_number}"),
                None::<&()>,
            )
            .await?;

        let stage = match issue.milestone.as_ref().map(|m| m.title.as_str()) {
            Some("PLANNING") => Stage::Planning,
            Some("PENDING") => Stage::Pending,
            Some("GO_PLANNING") => Stage::GoPlanning,
            Some("GO_WORKING") => Stage::GoWorking,
            Some("WORKING") => Stage::Working,
            _ => Stage::Planning, // default
        };

        let model = issue
            .labels
            .iter()
            .find_map(|l| l.name.strip_prefix("copilot:").map(String::from));

        let done = issue.labels.iter().any(|l| l.name == "done");

        Ok(Task {
            id: issue.number,
            title: issue.title,
            description: issue.body.unwrap_or_default(),
            stage,
            model,
            done,
        })
    }

    /// Get all comments on an issue as formatted discussion.
    pub(crate) async fn get_issue_comments(
        &self,
        issue_number: u64,
    ) -> Result<Vec<String>, ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let comments: Vec<CommentResponse> = self
            .octocrab
            .get(
                format!("/repos/{owner}/{repo}/issues/{issue_number}/comments"),
                None::<&()>,
            )
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

    /// Post a comment on an issue.
    pub(crate) async fn post_issue_comment(
        &self,
        issue_number: u64,
        body: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        self.octocrab
            .issues(owner, repo)
            .create_comment(issue_number, body)
            .await?;
        Ok(())
    }

    /// Set the milestone on an issue by milestone title.
    pub(crate) async fn set_issue_milestone(
        &self,
        issue_number: u64,
        milestone_title: &str,
    ) -> Result<(), ZbobrError> {
        let milestone_number = self
            .find_milestone_number(milestone_title)
            .await?
            .ok_or_else(|| {
                ZbobrError::GitHub(format!("Milestone '{milestone_title}' not found"))
            })?;

        let (owner, repo) = self.config.parse_repo()?;
        self.octocrab
            .patch(
                format!("/repos/{owner}/{repo}/issues/{issue_number}"),
                Some(&serde_json::json!({ "milestone": milestone_number })),
            )
            .await
            .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    /// Add a label to an issue.
    pub(crate) async fn add_issue_label(
        &self,
        issue_number: u64,
        label: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        self.octocrab
            .issues(owner, repo)
            .add_labels(issue_number, &[label.to_string()])
            .await?;
        Ok(())
    }

    /// Remove a label from an issue (ignores error if label not present).
    #[allow(dead_code)]
    pub(crate) async fn remove_issue_label(
        &self,
        issue_number: u64,
        label: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        // Removing a label that doesn't exist returns 404, which we ignore.
        let _ = self
            .octocrab
            .issues(owner, repo)
            .remove_label(issue_number, label)
            .await;
        Ok(())
    }

    /// Update the issue body (description).
    pub(crate) async fn update_issue_body(
        &self,
        issue_number: u64,
        body: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        self.octocrab
            .patch(
                format!("/repos/{owner}/{repo}/issues/{issue_number}"),
                Some(&serde_json::json!({ "body": body })),
            )
            .await
            .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    /// List open issues with a given milestone title.
    pub(crate) async fn list_issues_by_milestone(
        &self,
        milestone_title: &str,
    ) -> Result<Vec<Task>, ZbobrError> {
        let milestone_number = match self.find_milestone_number(milestone_title).await? {
            Some(n) => n,
            None => return Ok(vec![]),
        };

        let (owner, repo) = self.config.parse_repo()?;
        let issues: Vec<IssueResponse> = self
            .octocrab
            .get(
                format!("/repos/{owner}/{repo}/issues"),
                Some(&[
                    ("milestone", milestone_number.to_string().as_str()),
                    ("state", "open"),
                ]),
            )
            .await?;

        let mut tasks = Vec::new();
        for issue in issues {
            let stage = match issue.milestone.as_ref().map(|m| m.title.as_str()) {
                Some("PLANNING") => Stage::Planning,
                Some("PENDING") => Stage::Pending,
                Some("GO_PLANNING") => Stage::GoPlanning,
                Some("GO_WORKING") => Stage::GoWorking,
                Some("WORKING") => Stage::Working,
                _ => Stage::Planning,
            };
            let model = issue
                .labels
                .iter()
                .find_map(|l| l.name.strip_prefix("copilot:").map(String::from));
            let done = issue.labels.iter().any(|l| l.name == "done");
            tasks.push(Task {
                id: issue.number,
                title: issue.title,
                description: issue.body.unwrap_or_default(),
                stage,
                model,
                done,
            });
        }
        Ok(tasks)
    }

    /// Check if an issue is closed.
    pub(crate) async fn is_issue_closed(&self, issue_number: u64) -> Result<bool, ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let issue: IssueResponse = self
            .octocrab
            .get(
                format!("/repos/{owner}/{repo}/issues/{issue_number}"),
                None::<&()>,
            )
            .await?;
        Ok(issue.state == "closed")
    }
}

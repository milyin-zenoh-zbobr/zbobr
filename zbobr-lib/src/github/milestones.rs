use crate::{Zbobr, ZbobrError};

#[derive(Debug, serde::Deserialize)]
struct MilestoneResponse {
    number: u64,
    title: String,
}

impl Zbobr {
    /// List all milestones in the domain repo.
    pub(crate) async fn list_milestones(&self) -> Result<Vec<(u64, String)>, ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let milestones: Vec<MilestoneResponse> = self
            .octocrab
            .get(format!("/repos/{owner}/{repo}/milestones"), None::<&()>)
            .await?;
        Ok(milestones
            .into_iter()
            .map(|m| (m.number, m.title))
            .collect())
    }

    /// Create a milestone in the domain repo.
    pub(crate) async fn create_milestone(
        &self,
        title: &str,
        description: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        self.octocrab
            .post(
                format!("/repos/{owner}/{repo}/milestones"),
                Some(&serde_json::json!({
                    "title": title,
                    "description": description,
                    "state": "open"
                })),
            )
            .await
            .map(|_: serde_json::Value| ())?;
        Ok(())
    }

    /// Delete a milestone by its number.
    pub(crate) async fn delete_milestone(&self, number: u64) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let _response = self
            .octocrab
            ._delete(
                format!("/repos/{owner}/{repo}/milestones/{number}"),
                None::<&()>,
            )
            .await?;
        Ok(())
    }

    /// Find milestone number by title.
    pub(crate) async fn find_milestone_number(
        &self,
        title: &str,
    ) -> Result<Option<u64>, ZbobrError> {
        let milestones = self.list_milestones().await?;
        Ok(milestones
            .into_iter()
            .find(|(_, t)| t == title)
            .map(|(n, _)| n))
    }
}

use crate::{Zbobr, ZbobrError};

impl Zbobr {
    /// List all labels in the domain repo.
    pub(crate) async fn list_labels(&self) -> Result<Vec<String>, ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        let labels: Vec<octocrab::models::Label> = self
            .octocrab
            .issues(owner, repo)
            .list_labels_for_repo()
            .per_page(100)
            .send()
            .await?
            .items;
        Ok(labels.into_iter().map(|l| l.name).collect())
    }

    /// Create a label in the domain repo.
    pub(crate) async fn create_label(
        &self,
        name: &str,
        color: &str,
        description: &str,
    ) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        self.octocrab
            .issues(owner, repo)
            .create_label(name, color, description)
            .await?;
        Ok(())
    }

    /// Delete a label from the domain repo.
    #[allow(dead_code)]
    pub(crate) async fn delete_label(&self, name: &str) -> Result<(), ZbobrError> {
        let (owner, repo) = self.config.parse_repo()?;
        self.octocrab
            .issues(owner, repo)
            .delete_label(name)
            .await?;
        Ok(())
    }
}

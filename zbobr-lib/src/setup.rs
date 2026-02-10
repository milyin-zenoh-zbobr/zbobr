use crate::{Zbobr, ZbobrError};

impl Zbobr {
    /// Set up the domain repository: create if not exists, update stages and labels.
    pub async fn setup(&self, force: bool) -> Result<(), ZbobrError> {
        self.setup_repository(force).await
    }
}

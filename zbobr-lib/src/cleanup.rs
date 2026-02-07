use crate::{Zbobr, ZbobrError};

impl Zbobr {
    /// Clean up workspace directories for closed issues.
    /// If dry_run is true, only logs what would happen.
    pub async fn cleanup_closed_tasks(&self, dry_run: bool) -> Result<(), ZbobrError> {
        let workspace = &self.config.workspace;

        if !workspace.exists() {
            tracing::info!("Workspace directory does not exist: {}", workspace.display());
            return Ok(());
        }

        tracing::info!("Scanning workspace: {}", workspace.display());
        if dry_run {
            tracing::info!("DRY RUN - no files will be deleted");
        }

        let mut entries = tokio::fs::read_dir(workspace).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("issue#") {
                continue;
            }

            let issue_number: u64 = match name.strip_prefix("issue#").and_then(|s| s.parse().ok())
            {
                Some(n) => n,
                None => continue,
            };

            match self.is_issue_closed(issue_number).await {
                Ok(true) => {
                    let path = entry.path();
                    if dry_run {
                        tracing::info!("DRY RUN: Would remove {} (issue #{issue_number} is closed)", path.display());
                    } else {
                        tracing::info!(
                            "Removing {} (issue #{issue_number} is closed)",
                            path.display()
                        );
                        tokio::fs::remove_dir_all(&path).await?;
                    }
                }
                Ok(false) => {
                    tracing::info!(
                        "Issue #{issue_number} is open - keeping {}",
                        entry.path().display()
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to check issue #{issue_number}: {e} - skipping"
                    );
                }
            }
        }

        tracing::info!("Cleanup complete");
        Ok(())
    }
}

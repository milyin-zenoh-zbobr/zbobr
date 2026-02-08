use std::path::{Path, PathBuf};

use crate::{SetupFile, Zbobr, ZbobrError};

impl Zbobr {
    /// Stage 1: Write all setup files to a local directory.
    /// Always runs, even in dry-run mode. Creates the directory if needed.
    /// Returns the output directory path.
    pub async fn setup_write_local(
        &self,
        output_dir: &Path,
        files: &[SetupFile],
    ) -> Result<PathBuf, ZbobrError> {
        tokio::fs::create_dir_all(output_dir).await?;
        tracing::info!("Writing setup files to {}", output_dir.display());

        for file in files {
            let dest = output_dir.join(&file.path);
            // Create parent dirs if the file path has subdirectories
            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&dest, &file.content).await?;
            tracing::info!("  + {}", file.path);
        }

        tracing::info!("Local setup files written to {}", output_dir.display());
        Ok(output_dir.to_path_buf())
    }

    /// Stage 2: Push content to GitHub -- create repo, stages, labels, and files.
    /// Reads files from the local directory created by stage 1.
    /// If force is true, overwrites existing files and labels.
    pub async fn setup_push_remote(
        &self,
        _local_dir: &Path,
        files: &[SetupFile],
        force: bool,
    ) -> Result<(), ZbobrError> {
        self.setup_repository(files, force).await
    }
}

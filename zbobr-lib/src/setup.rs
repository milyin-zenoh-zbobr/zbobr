use std::path::{Path, PathBuf};

use crate::{Zbobr, ZbobrError, Stage};

/// Desired labels for the domain project.
const DONE_LABEL: &str = "done";
const DONE_LABEL_COLOR: &str = "5319e7";
const MODEL_LABEL_COLOR: &str = "bfd4f2";

/// Milestone descriptions.
fn milestone_description(stage: Stage) -> &'static str {
    match stage {
        Stage::Planning => "Issue is being planned by agent",
        Stage::Pending => "Issue plan is complete, awaiting human review or implementation is done",
        Stage::Ready => "Issue is approved and ready for worker agent",
        Stage::Working => "Issue is being implemented by worker agent",
    }
}

/// A file to create in the domain repository during setup.
pub struct SetupFile {
    /// Path relative to the repo root (e.g., "README.md").
    pub path: String,
    /// File content (plain text).
    pub content: String,
}

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

    /// Stage 2: Push content to GitHub -- create repo, milestones, labels, and files.
    /// Reads files from the local directory created by stage 1.
    pub async fn setup_push_remote(
        &self,
        local_dir: &Path,
        files: &[SetupFile],
    ) -> Result<(), ZbobrError> {
        tracing::info!("Pushing setup to GitHub: {}", self.config.domain_repo);

        // Ensure the domain repo exists
        self.ensure_domain_repo_exists().await?;

        // Create milestones
        let desired_stages = [Stage::Planning, Stage::Pending, Stage::Ready, Stage::Working];
        let existing = self.list_milestones().await?;
        let existing_titles: Vec<&str> = existing.iter().map(|(_, t)| t.as_str()).collect();

        for stage in &desired_stages {
            let title = stage.milestone_name();
            if existing_titles.contains(&title) {
                tracing::info!("Milestone '{title}' already exists");
            } else {
                tracing::info!("Creating milestone '{title}'");
                self.create_milestone(title, milestone_description(*stage))
                    .await?;
            }
        }

        // Delete extra milestones
        let desired_titles: Vec<&str> = desired_stages.iter().map(|s| s.milestone_name()).collect();
        for (number, title) in &existing {
            if !desired_titles.contains(&title.as_str()) {
                tracing::info!("Deleting milestone '{title}'");
                self.delete_milestone(*number).await?;
            }
        }

        // Create labels
        let existing_labels = self.list_labels().await?;

        if !existing_labels.contains(&DONE_LABEL.to_string()) {
            tracing::info!("Creating label '{DONE_LABEL}'");
            self.create_label(DONE_LABEL, DONE_LABEL_COLOR, "Issue implementation completed")
                .await?;
        } else {
            tracing::info!("Label '{DONE_LABEL}' already exists");
        }

        let model_label = format!("copilot:{}", self.config.default_model);
        if !existing_labels.contains(&model_label) {
            tracing::info!("Creating label '{model_label}'");
            self.create_label(
                &model_label,
                MODEL_LABEL_COLOR,
                &format!("Use {} model", self.config.default_model),
            )
            .await?;
        } else {
            tracing::info!("Label '{model_label}' already exists");
        }

        // Push files from local directory to GitHub
        for file in files {
            let local_path = local_dir.join(&file.path);
            let content = tokio::fs::read_to_string(&local_path).await.map_err(|e| {
                ZbobrError::Other(format!(
                    "Failed to read {}: {e}",
                    local_path.display()
                ))
            })?;

            let exists = self.repo_file_exists(&file.path).await?;
            if exists {
                tracing::info!("File '{}' already exists in repo, skipping", file.path);
            } else {
                tracing::info!("Pushing '{}' to repo", file.path);
                self.create_repo_file(
                    &file.path,
                    &content,
                    &format!("Initialize {} from zbobr setup", file.path),
                )
                .await?;
            }
        }

        tracing::info!("GitHub setup complete for {}", self.config.domain_repo);
        Ok(())
    }
}

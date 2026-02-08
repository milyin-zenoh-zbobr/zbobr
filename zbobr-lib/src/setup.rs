use std::path::{Path, PathBuf};

use crate::{Stage, Zbobr, ZbobrError};

/// Desired labels for the domain project.
const DONE_LABEL: &str = "done";
const DONE_LABEL_COLOR: &str = "5319e7";
const MODEL_LABEL_COLOR: &str = "bfd4f2";

/// Stage descriptions.
fn stage_description(stage: Stage) -> &'static str {
    match stage {
        Stage::Pending => "Task is under user's control, bot ignores it",
        Stage::PlanningReady => "Task must be taken by planner agent, any matching bot can take it",
        Stage::Planning => "Task is in planning, other bots ignore it",
        Stage::WorkingReady => "Task must be taken by worker agent, any matching bot can take it",
        Stage::Working => "Task is in work, other bots ignore it",
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

    /// Stage 2: Push content to GitHub -- create repo, stages, labels, and files.
    /// Reads files from the local directory created by stage 1.
    pub async fn setup_push_remote(
        &self,
        local_dir: &Path,
        files: &[SetupFile],
    ) -> Result<(), ZbobrError> {
        tracing::info!("Pushing setup to GitHub: {}", self.config.domain_repo);

        // Ensure the domain repo exists
        self.ensure_domain_repo_exists().await?;

        // Create stages
        let desired_stages = [
            Stage::Pending,
            Stage::PlanningReady,
            Stage::Planning,
            Stage::WorkingReady,
            Stage::Working,
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

        if !existing_labels.contains(&DONE_LABEL.to_string()) {
            tracing::info!("Creating label '{DONE_LABEL}'");
            self.create_label(
                DONE_LABEL,
                DONE_LABEL_COLOR,
                "Task implementation completed",
            )
            .await?;
        } else {
            tracing::info!("Label '{DONE_LABEL}' already exists");
        }

        let model_label = format!("model:{}", self.config.default_model);
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
                ZbobrError::Other(format!("Failed to read {}: {e}", local_path.display()))
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

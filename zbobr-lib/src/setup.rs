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

impl Zbobr {
    /// Set up the domain project: create labels and milestones.
    /// If dry_run is true, only logs what would happen.
    pub async fn setup_domain_project(&self, dry_run: bool) -> Result<(), ZbobrError> {
        tracing::info!("Setting up domain project: {}", self.config.domain_repo);

        // Setup milestones
        let desired_stages = [Stage::Planning, Stage::Pending, Stage::Ready, Stage::Working];
        let existing = self.list_milestones().await?;
        let existing_titles: Vec<&str> = existing.iter().map(|(_, t)| t.as_str()).collect();

        for stage in &desired_stages {
            let title = stage.milestone_name();
            if existing_titles.contains(&title) {
                tracing::info!("Milestone '{title}' already exists");
            } else if dry_run {
                tracing::info!("DRY RUN: Would create milestone '{title}'");
            } else {
                tracing::info!("Creating milestone '{title}'");
                self.create_milestone(title, milestone_description(*stage))
                    .await?;
            }
        }

        // Delete milestones that shouldn't exist
        let desired_titles: Vec<&str> = desired_stages.iter().map(|s| s.milestone_name()).collect();
        for (number, title) in &existing {
            if !desired_titles.contains(&title.as_str()) {
                if dry_run {
                    tracing::info!("DRY RUN: Would delete milestone '{title}'");
                } else {
                    tracing::info!("Deleting milestone '{title}'");
                    self.delete_milestone(*number).await?;
                }
            }
        }

        // Setup labels
        let existing_labels = self.list_labels().await?;

        // Ensure 'done' label
        if !existing_labels.contains(&DONE_LABEL.to_string()) {
            if dry_run {
                tracing::info!("DRY RUN: Would create label '{DONE_LABEL}'");
            } else {
                tracing::info!("Creating label '{DONE_LABEL}'");
                self.create_label(DONE_LABEL, DONE_LABEL_COLOR, "Issue implementation completed")
                    .await?;
            }
        } else {
            tracing::info!("Label '{DONE_LABEL}' already exists");
        }

        // Ensure default model label
        let model_label = format!("copilot:{}", self.config.default_model);
        if !existing_labels.contains(&model_label) {
            if dry_run {
                tracing::info!("DRY RUN: Would create label '{model_label}'");
            } else {
                tracing::info!("Creating label '{model_label}'");
                self.create_label(
                    &model_label,
                    MODEL_LABEL_COLOR,
                    &format!("Use {} model", self.config.default_model),
                )
                .await?;
            }
        } else {
            tracing::info!("Label '{model_label}' already exists");
        }

        tracing::info!("Domain project setup complete");
        Ok(())
    }
}

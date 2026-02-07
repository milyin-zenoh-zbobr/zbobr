use std::path::PathBuf;

use crate::ZbobrError;

/// Configuration for the zbobr orchestrator.
#[derive(Debug, Clone)]
pub struct ZbobrConfig {
    /// Domain project repository ("Org/repo").
    pub domain_repo: String,
    /// Owner for forks (user or org).
    pub fork_owner: String,
    /// Default AI model to use.
    pub default_model: String,
    /// Workspace directory for issue work dirs.
    pub workspace: PathBuf,
    /// GitHub personal access token.
    pub github_token: String,
}

impl ZbobrConfig {
    /// Load configuration from environment variables.
    ///
    /// Required: ZBOBR_DOMAIN_REPO, ZBOBR_FORK_OWNER, GH_TOKEN or GITHUB_TOKEN
    /// Optional: ZBOBR_DEFAULT_MODEL (default: "gpt-5-mini"), ZBOBR_WORKSPACE (default: "./workspace")
    pub fn from_env() -> Result<Self, ZbobrError> {
        let domain_repo = std::env::var("ZBOBR_DOMAIN_REPO")
            .map_err(|_| ZbobrError::Config("ZBOBR_DOMAIN_REPO not set".into()))?;

        let fork_owner = std::env::var("ZBOBR_FORK_OWNER")
            .map_err(|_| ZbobrError::Config("ZBOBR_FORK_OWNER not set".into()))?;

        let default_model = std::env::var("ZBOBR_DEFAULT_MODEL")
            .unwrap_or_else(|_| "gpt-5-mini".into());

        let workspace = std::env::var("ZBOBR_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./workspace"));

        let github_token = std::env::var("GH_TOKEN")
            .or_else(|_| std::env::var("GITHUB_TOKEN"))
            .map_err(|_| ZbobrError::Config("GH_TOKEN or GITHUB_TOKEN not set".into()))?;

        Ok(Self {
            domain_repo,
            fork_owner,
            default_model,
            workspace,
            github_token,
        })
    }

    /// Parse "owner/repo" into (owner, repo).
    pub fn parse_repo(&self) -> Result<(&str, &str), ZbobrError> {
        let parts: Vec<&str> = self.domain_repo.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(ZbobrError::Config(format!(
                "Invalid domain_repo format '{}', expected 'owner/repo'",
                self.domain_repo
            )));
        }
        Ok((parts[0], parts[1]))
    }
}

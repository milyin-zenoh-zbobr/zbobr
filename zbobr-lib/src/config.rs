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
    /// Required (can be provided later via CLI): ZBOBR_DOMAIN_REPO, ZBOBR_FORK_OWNER
    /// Required: GH_TOKEN or GITHUB_TOKEN
    /// Optional: ZBOBR_DEFAULT_MODEL (default: "gpt-5-mini"), ZBOBR_WORKSPACE (default: "./workspace")
    pub fn from_env() -> Result<Self, ZbobrError> {
        let domain_repo = std::env::var("ZBOBR_DOMAIN_REPO").unwrap_or_default();

        let fork_owner = std::env::var("ZBOBR_FORK_OWNER").unwrap_or_default();

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

    /// Validate that all required fields are set.
    pub fn validate(&self) -> Result<(), ZbobrError> {
        if self.domain_repo.is_empty() {
            return Err(ZbobrError::Config(
                "domain repo not set (use --domain-repo or ZBOBR_DOMAIN_REPO)".into(),
            ));
        }
        if self.fork_owner.is_empty() {
            return Err(ZbobrError::Config(
                "fork owner not set (use --fork-owner or ZBOBR_FORK_OWNER)".into(),
            ));
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(domain_repo: &str) -> ZbobrConfig {
        ZbobrConfig {
            domain_repo: domain_repo.to_string(),
            fork_owner: "test-fork".to_string(),
            default_model: "gpt-5-mini".to_string(),
            workspace: PathBuf::from("./workspace"),
            github_token: "fake-token".to_string(),
        }
    }

    #[test]
    fn parse_repo_valid() {
        let cfg = test_config("MyOrg/my-project");
        let (owner, repo) = cfg.parse_repo().unwrap();
        assert_eq!(owner, "MyOrg");
        assert_eq!(repo, "my-project");
    }

    #[test]
    fn parse_repo_with_slashes_in_name() {
        let cfg = test_config("owner/repo/extra");
        let (owner, repo) = cfg.parse_repo().unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo/extra");
    }

    #[test]
    fn parse_repo_invalid() {
        let cfg = test_config("no-slash-here");
        assert!(cfg.parse_repo().is_err());
    }

    #[test]
    fn from_env_missing_required() {
        // Clear env to ensure ZBOBR_DOMAIN_REPO is not set
        std::env::remove_var("ZBOBR_DOMAIN_REPO");
        let result = ZbobrConfig::from_env();
        assert!(result.is_err());
    }
}

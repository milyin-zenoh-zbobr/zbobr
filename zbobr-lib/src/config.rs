use std::path::PathBuf;

use crate::ZbobrError;

use crate::task::{Model, Tool};

/// Backend type to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    GitHub,
    Stub,
}

/// Configuration for the zbobr orchestrator.
#[derive(Debug, Clone)]
pub struct ZbobrConfig {
    /// Domain project repository ("Org/repo").
    pub domain_repo: String,
    /// Owner for forks (user or org).
    pub fork_owner: String,
    /// Default AI model to use.
    pub default_model: Model,
    /// Workspace directory for issue work dirs.
    pub workspace: PathBuf,
    /// GitHub personal access token.
    pub github_token: String,
    /// Backend to use.
    pub backend: BackendType,
    /// CLI tool to use.
    pub cli_tool: Tool,
    /// Semicolon-separated list of custom prompt files for planner agent.
    pub planner_prompts: Vec<PathBuf>,
    /// Semicolon-separated list of custom prompt files for worker agent.
    pub worker_prompts: Vec<PathBuf>,
}

impl ZbobrConfig {
    /// Load configuration from environment variables.
    ///
    /// Required (can be provided later via CLI): ZBOBR_DOMAIN_REPO, ZBOBR_FORK_OWNER
    /// Required: GH_TOKEN or GITHUB_TOKEN
    /// Optional: ZBOBR_DEFAULT_MODEL (default: "gpt-5-mini"), ZBOBR_WORKSPACE (default: "./workspace")
    /// Optional: ZBOBR_PLANNER_PROMPTS, ZBOBR_WORKER_PROMPTS (semicolon-separated file paths)
    pub fn from_env() -> Result<Self, ZbobrError> {
        let domain_repo = std::env::var("ZBOBR_DOMAIN_REPO").unwrap_or_default();

        let fork_owner = std::env::var("ZBOBR_FORK_OWNER").unwrap_or_default();

        let default_model_str =
            std::env::var("ZBOBR_DEFAULT_MODEL").unwrap_or_else(|_| "gpt-5-mini".into());
        let default_model = match default_model_str.as_str() {
            "gpt-4o" => Model::Gpt4o,
            "claude-3-5-sonnet" => Model::Claude35Sonnet,
            "claude-3-opus" => Model::Claude3Opus,
            _ => Model::Gpt5Mini,
        };

        let workspace = std::env::var("ZBOBR_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./workspace"));

        let github_token = std::env::var("GH_TOKEN")
            .or_else(|_| std::env::var("GITHUB_TOKEN"))
            .map_err(|_| ZbobrError::Config(
                "GitHub token not found. Set GH_TOKEN or GITHUB_TOKEN env var.\n  \
                 If you have GitHub CLI installed: export GH_TOKEN=$(gh auth token)\n  \
                 Otherwise create a token at https://github.com/settings/tokens (needs 'repo' scope)".into()
            ))?;

        let backend = match std::env::var("ZBOBR_BACKEND").unwrap_or_default().as_str() {
            "stub" => BackendType::Stub,
            _ => BackendType::GitHub,
        };

        let cli_tool = match std::env::var("ZBOBR_CLI_TOOL").unwrap_or_default().as_str() {
            "claude" => Tool::Claude,
            "stub" => Tool::Stub,
            _ => Tool::Copilot,
        };

        // Parse semicolon-separated prompt file paths
        let planner_prompts = std::env::var("ZBOBR_PLANNER_PROMPTS")
            .unwrap_or_else(|_| {
                "prompts/planner-workflow.md;prompts/repositories.md;prompts/common.md;prompts/planner.md".into()
            })
            .split(';')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();

        let worker_prompts = std::env::var("ZBOBR_WORKER_PROMPTS")
            .unwrap_or_else(|_| {
                "prompts/worker-workflow.md;prompts/common.md;prompts/worker.md".into()
            })
            .split(';')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();

        Ok(Self {
            domain_repo,
            fork_owner,
            default_model,
            workspace,
            github_token,
            backend,
            cli_tool,
            planner_prompts,
            worker_prompts,
        })
    }

    /// Validate that all required fields are set.
    pub fn validate(&self) -> Result<(), ZbobrError> {
        if self.domain_repo.is_empty() {
            return Err(ZbobrError::Config(
                "domain repo not set. Use --domain-repo owner/repo or set ZBOBR_DOMAIN_REPO.\n  \
                 This is the GitHub repository whose issues the orchestrator processes."
                    .into(),
            ));
        }
        if self.fork_owner.is_empty() {
            return Err(ZbobrError::Config(
                "fork owner not set. Use --fork-owner NAME or set ZBOBR_FORK_OWNER.\n  \
                 This is the GitHub user or organization where target repos are forked for implementation.".into(),
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
            default_model: Model::Gpt5Mini,
            workspace: PathBuf::from("./workspace"),
            github_token: "fake-token".to_string(),
            backend: BackendType::Stub,
            cli_tool: Tool::Copilot,
            planner_prompts: vec![],
            worker_prompts: vec![],
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
        // Ensure we have a token so we don't fail on "GitHub token not found"
        std::env::set_var("GH_TOKEN", "test-token");

        let config = ZbobrConfig::from_env().expect("from_env should succeed with token");
        // validate() should fail because domain_repo is missing
        assert!(config.validate().is_err());
    }
}

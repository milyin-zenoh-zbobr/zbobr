use std::path::PathBuf;

use crate::ZbobrError;

use crate::task::{Model, Tool};

/// Backend type to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BackendType {
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "stub")]
    Stub,
}

impl Default for BackendType {
    fn default() -> Self {
        BackendType::GitHub
    }
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::GitHub => write!(f, "github"),
            BackendType::Stub => write!(f, "stub"),
        }
    }
}

impl std::str::FromStr for BackendType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "github" => Ok(BackendType::GitHub),
            "stub" => Ok(BackendType::Stub),
            _ => Err(format!("Unknown backend: {}", s)),
        }
    }
}

/// TOML prompts configuration section.
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
pub struct TomlPrompts {
    pub path: Option<PathBuf>,
    pub planner: Option<Vec<PathBuf>>,
    pub worker: Option<Vec<PathBuf>>,
}

/// Configuration loaded from a TOML file.
/// All fields are optional — missing fields fall back to env vars or defaults.
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
pub struct TomlConfig {
    pub domain_repo: Option<String>,
    pub fork_owner: Option<String>,
    pub default_model: Option<Model>,
    pub workspace: Option<PathBuf>,
    pub github_token: Option<String>,
    pub backend: Option<BackendType>,
    pub cli_tool: Option<Tool>,
    pub work_branch_prefix: Option<String>,
    pub prompts: Option<TomlPrompts>,
}

impl TomlConfig {
    /// Load a TOML config from a file path.
    /// Returns Ok(None) if the file does not exist.
    pub fn load(path: &std::path::Path) -> Result<Option<Self>, ZbobrError> {
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| ZbobrError::Config(format!("Failed to read {}: {e}", path.display())))?;
        let config: TomlConfig = toml::from_str(&content)
            .map_err(|e| ZbobrError::Config(format!("Failed to parse {}: {e}", path.display())))?;
        Ok(Some(config))
    }
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
    /// Custom prompt files for planner agent.
    pub planner_prompts: Vec<PathBuf>,
    /// Custom prompt files for worker agent.
    pub worker_prompts: Vec<PathBuf>,
    /// Prefix for work branches (default: "zbobr_fix").
    pub work_branch_prefix: String,
    /// Base directory for resolving prompt file paths.
    pub prompts_path: Option<PathBuf>,
}

impl Default for ZbobrConfig {
    fn default() -> Self {
        Self {
            domain_repo: String::new(),
            fork_owner: String::new(),
            default_model: Model::default(),
            workspace: PathBuf::from("./workspace"),
            github_token: String::new(),
            backend: BackendType::default(),
            cli_tool: Tool::default(),
            planner_prompts: vec!["prompts/planner.md".into(), "prompts/common.md".into()],
            worker_prompts: vec!["prompts/worker.md".into(), "prompts/common.md".into()],
            work_branch_prefix: "zbobr_fix".to_string(),
            prompts_path: None,
        }
    }
}

/// Read an env var, falling back to a TOML value, then to a default.
fn env_or(env_key: &str, toml_val: Option<&str>, default: &str) -> String {
    std::env::var(env_key)
        .ok()
        .or_else(|| toml_val.map(String::from))
        .unwrap_or_else(|| default.to_string())
}

/// Read an env var and parse it via FromStr, returning None if unset or parse fails.
fn env_parsed<T: std::str::FromStr>(env_key: &str) -> Option<T> {
    std::env::var(env_key).ok().and_then(|v| v.parse().ok())
}

/// Read an env var as semicolon-separated PathBufs, returning None if unset.
fn env_path_list(env_key: &str) -> Option<Vec<PathBuf>> {
    std::env::var(env_key).ok().map(|v| {
        v.split(';')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect()
    })
}

impl ZbobrConfig {
    /// Build configuration by layering: defaults < TOML < env vars.
    ///
    /// Priority: env vars > TOML file > hardcoded defaults.
    pub fn build(toml: Option<&TomlConfig>) -> Result<Self, ZbobrError> {
        let defaults = ZbobrConfig::default();

        let domain_repo = env_or(
            "ZBOBR_DOMAIN_REPO",
            toml.and_then(|t| t.domain_repo.as_deref()),
            &defaults.domain_repo,
        );

        let fork_owner = env_or(
            "ZBOBR_FORK_OWNER",
            toml.and_then(|t| t.fork_owner.as_deref()),
            &defaults.fork_owner,
        );

        let default_model = env_parsed::<Model>("ZBOBR_DEFAULT_MODEL")
            .or_else(|| toml.and_then(|t| t.default_model.clone()))
            .unwrap_or(defaults.default_model);

        let workspace = std::env::var("ZBOBR_WORKSPACE")
            .ok()
            .map(PathBuf::from)
            .or_else(|| toml.and_then(|t| t.workspace.clone()))
            .unwrap_or(defaults.workspace);

        let github_token = std::env::var("GH_TOKEN")
            .or_else(|_| std::env::var("GITHUB_TOKEN"))
            .ok()
            .or_else(|| toml.and_then(|t| t.github_token.clone()))
            .unwrap_or_default();

        let backend = env_parsed::<BackendType>("ZBOBR_BACKEND")
            .or_else(|| toml.and_then(|t| t.backend))
            .unwrap_or(defaults.backend);

        let cli_tool = env_parsed::<Tool>("ZBOBR_CLI_TOOL")
            .or_else(|| toml.and_then(|t| t.cli_tool))
            .unwrap_or(defaults.cli_tool);

        let work_branch_prefix = env_or(
            "ZBOBR_WORK_BRANCH_PREFIX",
            toml.and_then(|t| t.work_branch_prefix.as_deref()),
            &defaults.work_branch_prefix,
        );

        let toml_prompts = toml.and_then(|t| t.prompts.as_ref());

        let planner_prompts = env_path_list("ZBOBR_PLANNER_PROMPTS")
            .or_else(|| toml_prompts.and_then(|p| p.planner.clone()))
            .unwrap_or(defaults.planner_prompts);

        let worker_prompts = env_path_list("ZBOBR_WORKER_PROMPTS")
            .or_else(|| toml_prompts.and_then(|p| p.worker.clone()))
            .unwrap_or(defaults.worker_prompts);

        let prompts_path = std::env::var("ZBOBR_PROMPTS_PATH")
            .ok()
            .map(PathBuf::from)
            .or_else(|| toml_prompts.and_then(|p| p.path.clone()));

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
            work_branch_prefix,
            prompts_path,
        })
    }

    /// Load configuration from environment variables only (backward compat).
    pub fn from_env() -> Result<Self, ZbobrError> {
        Self::build(None)
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
            work_branch_prefix: "zbobr_fix".to_string(),
            prompts_path: None,
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

    #[test]
    fn toml_config_parse_minimal() {
        let toml_str = r#"
domain_repo = "org/repo"
fork_owner = "myuser"
"#;
        let config: TomlConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.domain_repo.as_deref(), Some("org/repo"));
        assert_eq!(config.fork_owner.as_deref(), Some("myuser"));
        assert!(config.default_model.is_none());
        assert!(config.backend.is_none());
    }

    #[test]
    fn toml_config_parse_full() {
        let toml_str = r#"
domain_repo = "org/repo"
fork_owner = "myuser"
default_model = "gpt-5-mini"
workspace = "/tmp/workspace"
backend = "stub"
cli_tool = "claude"
work_branch_prefix = "my_fix"

[prompts]
path = "/opt/prompts"
planner = ["plan.md", "shared.md"]
worker = ["work.md"]
"#;
        let config: TomlConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.domain_repo.as_deref(), Some("org/repo"));
        assert_eq!(config.default_model, Some(Model::Gpt5Mini));
        assert_eq!(config.backend, Some(BackendType::Stub));
        assert_eq!(config.cli_tool, Some(Tool::Claude));
        let prompts = config.prompts.unwrap();
        assert_eq!(prompts.path, Some(PathBuf::from("/opt/prompts")));
        assert_eq!(prompts.planner, Some(vec![PathBuf::from("plan.md"), PathBuf::from("shared.md")]));
    }

    #[test]
    fn build_with_toml() {
        // Clear env vars that could interfere
        std::env::remove_var("ZBOBR_DOMAIN_REPO");
        std::env::remove_var("ZBOBR_FORK_OWNER");
        std::env::remove_var("ZBOBR_DEFAULT_MODEL");
        std::env::remove_var("ZBOBR_WORKSPACE");
        std::env::remove_var("ZBOBR_BACKEND");
        std::env::remove_var("ZBOBR_CLI_TOOL");
        std::env::remove_var("ZBOBR_WORK_BRANCH_PREFIX");
        std::env::remove_var("ZBOBR_PLANNER_PROMPTS");
        std::env::remove_var("ZBOBR_WORKER_PROMPTS");
        std::env::remove_var("ZBOBR_PROMPTS_PATH");
        std::env::set_var("GH_TOKEN", "test-token");

        let toml = TomlConfig {
            domain_repo: Some("toml-org/toml-repo".into()),
            fork_owner: Some("toml-fork".into()),
            default_model: Some(Model::Claude3Opus),
            workspace: Some(PathBuf::from("/tmp/toml-ws")),
            github_token: None,
            backend: Some(BackendType::Stub),
            cli_tool: Some(Tool::Claude),
            work_branch_prefix: Some("toml_fix".into()),
            prompts: Some(TomlPrompts {
                path: Some(PathBuf::from("/opt/prompts")),
                planner: Some(vec![PathBuf::from("p.md")]),
                worker: Some(vec![PathBuf::from("w.md")]),
            }),
        };

        let config = ZbobrConfig::build(Some(&toml)).unwrap();
        assert_eq!(config.domain_repo, "toml-org/toml-repo");
        assert_eq!(config.fork_owner, "toml-fork");
        assert_eq!(config.default_model, Model::Claude3Opus);
        assert_eq!(config.workspace, PathBuf::from("/tmp/toml-ws"));
        assert_eq!(config.backend, BackendType::Stub);
        assert_eq!(config.cli_tool, Tool::Claude);
        assert_eq!(config.work_branch_prefix, "toml_fix");
        assert_eq!(config.planner_prompts, vec![PathBuf::from("p.md")]);
        assert_eq!(config.worker_prompts, vec![PathBuf::from("w.md")]);
        assert_eq!(config.prompts_path, Some(PathBuf::from("/opt/prompts")));
    }

    #[test]
    fn build_defaults_without_toml() {
        std::env::remove_var("ZBOBR_DOMAIN_REPO");
        std::env::remove_var("ZBOBR_FORK_OWNER");
        std::env::remove_var("ZBOBR_DEFAULT_MODEL");
        std::env::remove_var("ZBOBR_WORKSPACE");
        std::env::remove_var("ZBOBR_BACKEND");
        std::env::remove_var("ZBOBR_CLI_TOOL");
        std::env::remove_var("ZBOBR_WORK_BRANCH_PREFIX");
        std::env::remove_var("ZBOBR_PLANNER_PROMPTS");
        std::env::remove_var("ZBOBR_WORKER_PROMPTS");
        std::env::remove_var("ZBOBR_PROMPTS_PATH");
        std::env::set_var("GH_TOKEN", "test-token");

        let config = ZbobrConfig::build(None).unwrap();
        assert_eq!(config.default_model, Model::Gpt5Mini);
        assert_eq!(config.backend, BackendType::GitHub);
        assert_eq!(config.cli_tool, Tool::Copilot);
        assert_eq!(config.work_branch_prefix, "zbobr_fix");
        assert_eq!(config.workspace, PathBuf::from("./workspace"));
    }

    #[test]
    fn backend_type_roundtrip() {
        assert_eq!("github".parse::<BackendType>().unwrap(), BackendType::GitHub);
        assert_eq!("stub".parse::<BackendType>().unwrap(), BackendType::Stub);
        assert!("invalid".parse::<BackendType>().is_err());
        assert_eq!(BackendType::GitHub.to_string(), "github");
        assert_eq!(BackendType::Stub.to_string(), "stub");
    }
}

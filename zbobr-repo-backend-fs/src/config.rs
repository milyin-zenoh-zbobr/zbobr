use std::path::{Path, PathBuf};

use zbobr_utility::{config_struct, resolve_path};

#[derive(Clone)]
#[config_struct]
/// Configuration for the filesystem repo backend.
pub struct ZbobrRepoBackendFs {
    #[arg(long)]
    pub repos_dir: PathBuf,
    #[arg(long)]
    pub repos_base_dir: Option<PathBuf>,
}

/// Resolved configuration for the filesystem repo backend.
impl Default for ZbobrRepoBackendFsConfig {
    fn default() -> Self {
        Self {
            repos_dir: PathBuf::from("./repos"),
            repos_base_dir: None,
        }
    }
}

impl ZbobrRepoBackendFsConfig {
    /// Build configuration by layering: defaults < TOML < args.
    /// Relative paths from TOML are resolved against `config_dir`.
    pub(crate) fn build(
        toml: Option<ZbobrRepoBackendFsToml>,
        args: ZbobrRepoBackendFsArgs,
        config_dir: &Path,
    ) -> Self {
        let defaults = Self::default();
        let merged = toml.unwrap_or_default().merge_with_args(args);

        let repos_dir = merged
            .repos_dir
            .map(|p| resolve_path(p, config_dir))
            .unwrap_or(defaults.repos_dir);

        let repos_base_dir = merged.repos_base_dir.map(|p| resolve_path(p, config_dir));

        Self {
            repos_dir,
            repos_base_dir,
        }
    }

    /// Validate that all required fields are set.
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        // repos_dir can be any path — we'll create it if needed
        Ok(())
    }
}

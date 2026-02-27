use std::path::{Path, PathBuf};
use std::sync::Arc;
use zbobr_dispatcher::Stage;

// ---------------------------------------------------------------------------
// Backend argument bundles
// ---------------------------------------------------------------------------

// variants may be unused depending on which integration-test binary is built
// (e.g. the filesystem-only suite never constructs the `GitHub` variants), so
// silence dead-code warnings here rather than peppering every test file with
// `#[allow(dead_code)]`.

#[allow(dead_code)]
pub enum TaskBackendArgs {
    Filesystem { tasks_dir: PathBuf },
    GitHub { task_repo: String, task_token: String },
}

#[allow(dead_code)]
pub enum RepoBackendArgs {
    Filesystem,
    GitHub { fork_owner: String, repo_token: String },
}

// ---------------------------------------------------------------------------
// Shared environment
// ---------------------------------------------------------------------------

/// All shared paths and state that survive for the entire integration-test
/// binary run.  Created once via a module-level `OnceCell` in each test file
/// and re-used by every test function within that binary.
///
/// **Important:** this environment uses *only* the CLI to manipulate tasks and
/// never instantiates backend implementations directly, so the same test logic
/// works with any combination of task and repo backends.
pub struct IntegrationTestEnv {
    /// Root directory for this run (workspaces, scenarios, scratch repos).
    pub base_path: PathBuf,
    pub workspaces_dir: PathBuf,
    /// Short label used in assertion messages ("fs_fs", "github_fs", …).
    pub name: &'static str,
    task_backend: TaskBackendArgs,
    repo_backend: RepoBackendArgs,
    agent_token: String,
    /// Optional GitHub repository slug (e.g. `"zbobr/test_tasks"`) used by
    /// repo-backend tests that need to clone a real remote repo.
    pub target_repo: Option<String>,
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl IntegrationTestEnv {
    /// Build and run `zbobr setup` for the given backend combination.
    ///
    /// Returns `None` when:
    /// - `mcp-tester` is not installed, or
    /// - `zbobr setup` fails (e.g. invalid GitHub credentials).
    pub async fn init(
        name: &'static str,
        task_backend: TaskBackendArgs,
        repo_backend: RepoBackendArgs,
        agent_token: String,
        target_repo: Option<String>,
    ) -> Option<Arc<Self>> {
        if !check_mcp_tester().await {
            return None;
        }

        let base_path = make_base_path(name).await;
        let workspaces_dir = base_path.join("workspaces");

        eprintln!("[IntegrationTestEnv/{name}] base path: {}", base_path.display());

        let env = Arc::new(IntegrationTestEnv {
            base_path,
            workspaces_dir,
            name,
            task_backend,
            repo_backend,
            agent_token,
            target_repo,
        });

        // For GitHub backends, setup can fail if credentials are invalid.
        // Catch the failure gracefully so the test binary skips cleanly.
        let zbobr_bin = env!("CARGO_BIN_EXE_zbobr");
        let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        let mut args = zbobr_config_args(&env);
        args.push("setup".to_string());

        let status = tokio::process::Command::new(zbobr_bin)
            .args(&args)
            .current_dir(&env.base_path)
            .env("RUST_LOG", &rust_log)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await
            .ok()?;

        if !status.success() {
            eprintln!(
                "[IntegrationTestEnv/{name}] `zbobr setup` failed — skipping. \
                 Check credentials in zbobr_github_test.toml."
            );
            return None;
        }

        Some(env)
    }

    // -----------------------------------------------------------------------
    // Identification
    // -----------------------------------------------------------------------

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Return the fork owner configured for the GitHub repo backend, or `None`
    /// when the filesystem backend is in use.
    pub fn fork_owner(&self) -> Option<&str> {
        match &self.repo_backend {
            RepoBackendArgs::GitHub { fork_owner, .. } => Some(fork_owner),
            RepoBackendArgs::Filesystem => None,
        }
    }

    // -----------------------------------------------------------------------
    // Task utilities
    // -----------------------------------------------------------------------

    /// Create a new task via the CLI and return its ID (confirm=false).
    pub async fn create_task(&self, title: &str, description: &str, stage: Stage) -> u64 {
        self.create_task_with_confirm(title, description, stage, false)
            .await
    }

    /// Create a new task via the CLI and return its ID, optionally enabling
    /// the `--confirm` flag when true.
    pub async fn create_task_with_confirm(
        &self,
        title: &str,
        description: &str,
        stage: Stage,
        confirm: bool,
    ) -> u64 {
        let stage_str = stage.to_string();
        let mut args: Vec<&str> = vec![
            "create",
            title,
            "--description",
            description,
            "--stage",
            &stage_str,
        ];
        if confirm {
            args.push("--confirm");
        }

        let output = self.run_zbobr_capture("task", &args).await;

        let line = output
            .lines()
            .find(|l| l.trim().starts_with("Created task #"))
            .unwrap_or_default();
        line.trim()
            .strip_prefix("Created task #")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_else(|| panic!("[{}] failed to parse task id from: {output}", self.name))
    }

    /// Show a task and return its raw CLI output.
    pub async fn show_task(&self, task_id: u64) -> String {
        self.run_zbobr_capture("task", &["show", &task_id.to_string()])
            .await
    }

    /// Update a task's repository and branch information.
    pub async fn update_task_branches(
        &self,
        task_id: u64,
        dest_repo: &str,
        dest_branch: &str,
        work_branch: &str,
    ) {
        let task_id_str = task_id.to_string();
        self.run_zbobr(
            "task",
            &[
                "update",
                &task_id_str,
                "--dest-repo",
                dest_repo,
                "--dest-branch",
                dest_branch,
                "--work-branch",
                work_branch,
            ],
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn task_stage(&self, task_id: u64) -> Stage {
        let output = self.show_task(task_id).await;
        let stage_line = output
            .lines()
            .find(|l| l.trim_start().starts_with("Stage:"))
            .unwrap_or_else(|| panic!("[{}] Stage line not found in: {output}", self.name));
        let stage_value = stage_line
            .split(':')
            .nth(1)
            .map(str::trim)
            .unwrap_or_else(|| panic!("[{}] Malformed stage line: {stage_line}", self.name));
        Stage::from_milestone_name(stage_value)
            .unwrap_or_else(|| panic!("[{}] Unknown stage '{stage_value}'", self.name))
    }

    // -----------------------------------------------------------------------
    // Git utilities
    // -----------------------------------------------------------------------

    /// Create a minimal local git repository under `base_path/<name>`.
    ///
    /// Returns the path of the new repository.  The path is used both as the
    /// workspace source (cloned locally) and stored as the `dest_repo`
    /// parameter in the task.  Integration tests exercise MCP tool behaviour
    /// (task storage) rather than actual push/PR operations, so a local path
    /// is always sufficient as the repository identifier.
    pub async fn create_git_repo(&self, name: &str) -> PathBuf {
        let repo_dir = self.base_path.join(name);
        tokio::fs::create_dir_all(&repo_dir).await.unwrap();

        async fn git(dir: &PathBuf, args: &[&str]) {
            let status = tokio::process::Command::new("git")
                .args(args)
                .current_dir(dir)
                .status()
                .await
                .unwrap();
            assert!(status.success(), "git {:?} failed", args);
        }

        git(&repo_dir, &["init"]).await;
        git(&repo_dir, &["config", "user.name", "test-bot"]).await;
        git(&repo_dir, &["config", "user.email", "test@example.com"]).await;
        tokio::fs::write(repo_dir.join("README.md"), "test repo")
            .await
            .unwrap();
        git(&repo_dir, &["add", "README.md"]).await;
        git(&repo_dir, &["commit", "-m", "Initial commit"]).await;
        git(&repo_dir, &["branch", "-M", "main"]).await;

        repo_dir
    }

    // -----------------------------------------------------------------------
    // Workspace helpers
    // -----------------------------------------------------------------------

    /// Clone `repo_path` into a per-task workspace directory and check out
    /// `work_branch` (creating it if absent).  Returns the clone path.
    pub async fn prepare_workspace(
        &self,
        task_id: u64,
        repo_path: &Path,
        work_branch: &str,
    ) -> PathBuf {
        let workspace_dir = self.workspaces_dir.join(format!("task#{task_id}"));
        tokio::fs::create_dir_all(&workspace_dir)
            .await
            .expect("failed to create workspace dir");

        let repo_name = repo_path
            .file_name()
            .expect("repo_path must have a file name")
            .to_str()
            .unwrap();
        let work_dir = workspace_dir.join(repo_name);

        let clone_ok = tokio::process::Command::new("git")
            .args(["clone", repo_path.to_str().unwrap(), work_dir.to_str().unwrap()])
            .status()
            .await
            .expect("failed to run git clone")
            .success();
        assert!(clone_ok, "[{}] git clone failed", self.name);

        let checkout = tokio::process::Command::new("git")
            .args(["checkout", work_branch])
            .current_dir(&work_dir)
            .output()
            .await
            .expect("failed to run git checkout");

        if !checkout.status.success() {
            let ok = tokio::process::Command::new("git")
                .args(["checkout", "-b", work_branch])
                .current_dir(&work_dir)
                .status()
                .await
                .expect("failed to run git checkout -b")
                .success();
            assert!(ok, "[{}] failed to create branch '{work_branch}'", self.name);
        }

        work_dir
    }

    /// Clone `repo_slug` into a per-task workspace directory via the configured
    /// repo backend (`zbobr task clone`), then check out `work_branch`
    /// (creating it if absent).  Returns the clone path.
    ///
    /// Unlike `prepare_workspace`, this method exercises the real repo backend
    /// (e.g. `clone_and_setup` for the GitHub backend) rather than calling
    /// `git clone` directly.  The task must already have its `dest_repo` and
    /// `dest_branch` parameters set before calling this.
    pub async fn prepare_workspace_via_repo_backend(
        &self,
        task_id: u64,
        repo_slug: &str,
        work_branch: &str,
    ) -> PathBuf {
        self.run_zbobr("task", &["clone", &task_id.to_string()])
            .await;

        let repo_name = repo_slug.rsplit('/').next().unwrap_or(repo_slug);
        let work_dir = self
            .workspaces_dir
            .join(format!("task#{task_id}"))
            .join(repo_name);

        assert!(
            work_dir.exists(),
            "[{}] Workspace directory missing after clone: {}",
            self.name,
            work_dir.display()
        );

        // Create/checkout the work branch
        let checkout = tokio::process::Command::new("git")
            .args(["checkout", work_branch])
            .current_dir(&work_dir)
            .output()
            .await
            .expect("failed to run git checkout");

        if !checkout.status.success() {
            let ok = tokio::process::Command::new("git")
                .args(["checkout", "-b", work_branch])
                .current_dir(&work_dir)
                .status()
                .await
                .expect("failed to run git checkout -b")
                .success();
            assert!(
                ok,
                "[{}] failed to create branch '{}' in workspace",
                self.name, work_branch
            );
        }

        work_dir
    }

    // -----------------------------------------------------------------------
    // zbobr CLI wrappers
    // -----------------------------------------------------------------------

    /// Execute `zbobr <command> [args…]` with inherited stdio.
    pub async fn run_zbobr(&self, command: &str, args: &[&str]) {
        let zbobr_bin = env!("CARGO_BIN_EXE_zbobr");
        let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        let mut full = zbobr_config_args(self);
        full.push(command.to_string());
        full.extend(args.iter().map(|s| s.to_string()));

        let status = tokio::process::Command::new(zbobr_bin)
            .args(&full)
            .current_dir(&self.base_path)
            .env("RUST_LOG", &rust_log)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await
            .expect("failed to run zbobr");

        assert!(
            status.success(),
            "[{}] zbobr {command} failed: {:?}",
            self.name,
            status.code()
        );
    }

    /// Execute `zbobr <command> [args…]` and return captured stdout.
    pub async fn run_zbobr_capture(&self, command: &str, args: &[&str]) -> String {
        let zbobr_bin = env!("CARGO_BIN_EXE_zbobr");
        let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        let mut full = zbobr_config_args(self);
        full.push(command.to_string());
        full.extend(args.iter().map(|s| s.to_string()));

        let output = tokio::process::Command::new(zbobr_bin)
            .args(&full)
            .current_dir(&self.base_path)
            .env("RUST_LOG", &rust_log)
            .output()
            .await
            .expect("failed to run zbobr");

        assert!(
            output.status.success(),
            "[{}] zbobr {command} failed: {:?}",
            self.name,
            output.status.code()
        );

        String::from_utf8_lossy(&output.stdout).to_string()
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn zbobr_config_args(env: &IntegrationTestEnv) -> Vec<String> {
    let mut args = Vec::new();
    let mut push = |flag: &str, val: &str| {
        args.push(flag.to_string());
        args.push(val.to_string());
    };

    push("--dispatcher-workspaces", &env.workspaces_dir.to_string_lossy());
    push("--dispatcher-cli-tool", "mcp-tester");
    push("--dispatcher-git-user-name", "test-bot");
    push("--dispatcher-git-user-email", "test@example.com");
    push("--dispatcher-agent-github-token", &env.agent_token);

    match &env.task_backend {
        TaskBackendArgs::Filesystem { tasks_dir } => {
            push("--dispatcher-task-backend", "filesystem");
            push("--tasks-fs-tasks-dir", &tasks_dir.to_string_lossy());
        }
        TaskBackendArgs::GitHub { task_repo, task_token } => {
            push("--dispatcher-task-backend", "github");
            push("--tasks-github-task-repo", task_repo);
            push("--tasks-github-token", task_token);
        }
    }

    match &env.repo_backend {
        RepoBackendArgs::Filesystem => {
            push("--dispatcher-repo-backend", "filesystem");
        }
        RepoBackendArgs::GitHub { fork_owner, repo_token } => {
            push("--dispatcher-repo-backend", "github");
            push("--repo-github-fork-owner", fork_owner);
            push("--repo-github-token", repo_token);
        }
    }

    args
}

async fn check_mcp_tester() -> bool {
    let result = tokio::process::Command::new("mcp-tester")
        .arg("--version")
        .output()
        .await;
    match result {
        Ok(out) if out.status.success() => true,
        _ => {
            eprintln!(
                "Skipping integration tests: mcp-tester not installed \
                 (cargo install mcp-tester)"
            );
            false
        }
    }
}

async fn make_base_path(name: &str) -> PathBuf {
    let base = match std::env::var("CARGO_TARGET_TMPDIR") {
        Ok(p) => PathBuf::from(p).join(format!("integration_env_{name}")),
        Err(_) => std::env::temp_dir().join(format!("zbobr_integration_env_{name}")),
    };
    if base.exists() {
        tokio::fs::remove_dir_all(&base)
            .await
            .expect("failed to clean previous integration env");
    }
    tokio::fs::create_dir_all(&base)
        .await
        .expect("failed to create integration env base dir");
    base
}

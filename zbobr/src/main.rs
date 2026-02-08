use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use zbobr_lib::task::{Model, Role, Tool};
use zbobr_lib::{SetupFile, Stage, Zbobr, ZbobrConfig};

#[derive(Args, Clone)]
struct GlobalArgs {
    /// Domain repository with tasks to orchestrate, in "owner/repo" format
    /// (e.g. "YoroolGui/copilot-zenoh"). Can also be set via ZBOBR_DOMAIN_REPO env var
    #[arg(long, global = true)]
    domain_repo: Option<String>,

    /// GitHub user or organization where target repos are forked for implementation
    /// (e.g. "YoroolGui"). Workers fork repos here to create PRs.
    /// Can also be set via ZBOBR_FORK_OWNER env var
    #[arg(long, global = true)]
    fork_owner: Option<String>,

    /// Path to workspace directory (default: ./workspace)
    /// Can also be set via ZBOBR_WORKSPACE env var
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,

    /// Semicolon-separated list of prompt files for planner role
    #[arg(long, env = "ZBOBR_PLANNER_PROMPTS", global = true, value_delimiter = ';')]
    planner_prompts: Option<Vec<PathBuf>>,

    /// Semicolon-separated list of prompt files for worker role
    #[arg(long, env = "ZBOBR_WORKER_PROMPTS", global = true, value_delimiter = ';')]
    worker_prompts: Option<Vec<PathBuf>>,

    /// Backend to use: "github" (default) or "stub"
    #[arg(long, global = true, env = "ZBOBR_BACKEND")]
    backend: Option<String>,

    /// CLI tool to use: "copilot", "claude", or "stub"
    #[arg(long, global = true, env = "ZBOBR_CLI_TOOL")]
    cli_tool: Option<String>,

    /// Port for the Admin MCP server (optional)
    #[arg(long, global = true, env = "ZBOBR_ADMIN_PORT")]
    admin_port: Option<u16>,
}

#[derive(Parser)]
#[command(
    name = "zbobr",
    about = "AI-powered task orchestrator",
    long_about = "AI-powered task orchestrator that manages tasks through automated stages.\n\n\
        Tasks flow through: PENDING -> PLANNING_READY -> PLANNING -> WORKING_READY -> WORKING.\n\
        Planner roles create implementation plans, worker roles implement them\n\
        by forking target repositories and creating pull requests.\n\n\
        Requires a GitHub token: set GH_TOKEN or GITHUB_TOKEN env var.\n\
        Easiest way: export GH_TOKEN=$(gh auth token)"
)]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a domain project: create config files locally and push to GitHub
    Setup {
        /// Only create local files, skip pushing to GitHub
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Output directory for local setup files (default: ./<repo-name>)
        #[arg(long, short = 'o')]
        output_dir: Option<PathBuf>,

        /// Force overwrite existing files and labels in the repository
        #[arg(long, short = 'f')]
        force: bool,
    },
    /// Poll for tasks and run planner/worker roles automatically
    Loop {
        /// How often to check for new tasks, in seconds
        #[arg(long, default_value = "60")]
        interval: u64,
        /// How often to clean up workspaces for closed tasks, in seconds
        #[arg(long, default_value = "600")]
        cleanup_interval: u64,
        /// AI model to use (e.g. "gpt-5-mini", "claude-3-5-sonnet")
        #[arg(long)]
        model: Option<String>,
        /// Port for the MCP server that roles connect to
        #[arg(long, default_value = "3000")]
        port: u16,
    },
    /// Remove workspace directories for tasks that have been closed
    Cleanup {
        /// Show what would be removed without actually deleting
        #[arg(long, short = 'n')]
        dry_run: bool,
    },
    /// Run planner role for a specific task (creates implementation plan)
    Plan {
        /// Task ID
        task: u64,
        /// AI model to use (e.g. "gpt-5-mini", "claude-3-5-sonnet")
        #[arg(long)]
        model: Option<String>,
        /// Port for the MCP server that the agent connects to
        #[arg(long, default_value = "3000")]
        port: u16,
    },
    /// Run worker role for a specific task (implements the plan, creates PR)
    Work {
        /// Task ID
        task: u64,
        /// AI model to use (e.g. "gpt-5-mini", "claude-3-5-sonnet")
        #[arg(long)]
        model: Option<String>,
        /// Port for the MCP server that the agent connects to
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

/// Resolved prompt file paths for planner and worker.
struct Prompts {
    planner: Vec<PathBuf>,
    worker: Vec<PathBuf>,
}

/// Resolve prompt paths: CLI arg > config env var.
/// Paths are resolved relative to current directory.
fn resolve_prompts(cli: &Cli, config: &ZbobrConfig) -> anyhow::Result<Prompts> {
    // Use CLI args if provided, otherwise use config (which came from env or defaults)
    let planner = cli
        .global
        .planner_prompts
        .clone()
        .unwrap_or_else(|| config.planner_prompts.clone());

    let worker = cli
        .global
        .worker_prompts
        .clone()
        .unwrap_or_else(|| config.worker_prompts.clone());

    Ok(Prompts { planner, worker })
}

/// Load and concatenate multiple prompt files.
fn load_prompts(paths: &[PathBuf]) -> anyhow::Result<String> {
    let mut combined = String::new();
    for (i, path) in paths.iter().enumerate() {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read prompt file {}: {e}", path.display()))?;

        if i > 0 {
            combined.push_str("\n\n");
        }
        combined.push_str(&content);
    }
    Ok(combined)
}

fn load_config(cli: &Cli) -> anyhow::Result<ZbobrConfig> {
    let mut config = ZbobrConfig::from_env()?;
    if let Some(ref dr) = cli.global.domain_repo {
        config.domain_repo = dr.clone();
    }
    if let Some(ref fo) = cli.global.fork_owner {
        config.fork_owner = fo.clone();
    }
    if let Some(ref ws) = cli.global.workspace {
        config.workspace = ws.clone();
    }
    if let Some(ref b) = cli.global.backend {
        match b.as_str() {
            "stub" => config.backend = zbobr_lib::config::BackendType::Stub,
            "github" => config.backend = zbobr_lib::config::BackendType::GitHub,
            _ => return Err(anyhow::anyhow!("Unknown backend: {}", b)),
        }
    }
    if let Some(ref t) = cli.global.cli_tool {
        config.cli_tool = t.parse::<Tool>().map_err(|e| anyhow::anyhow!(e))?;
    }
    config.validate()?;
    Ok(config)
}

/// Default output directory for setup: `./<repo-name>` in the current dir.
fn default_setup_dir(zbobr: &Zbobr) -> PathBuf {
    let repo = &zbobr.config().domain_repo;
    // Use the repo part after the slash, e.g. "Org/my-project" -> "my-project"
    let name = repo.split('/').nth(1).unwrap_or(repo);
    std::env::temp_dir().join(name)
}

/// Default resources directory: `resources/` next to the executable.
fn default_resources_dir() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine executable directory"))?;
    Ok(exe_dir.join("resources"))
}

/// Load a resource file from the resources directory.
fn load_resource(name: &str) -> anyhow::Result<String> {
    let path = default_resources_dir()?.join(name);
    std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read resource {}: {e}", path.display()))
}

/// Build the list of files to create in the domain repo during setup.
/// Template placeholders like {{DOMAIN_REPO}} are replaced with actual config values.
fn build_setup_files(zbobr: &Zbobr) -> anyhow::Result<Vec<SetupFile>> {
    let config = zbobr.config();

    let readme = load_resource("README.md")?;
    let run_sh = load_resource("run.sh")?;
    let run_cmd = load_resource("run.cmd")?;

    let env_template = load_resource("zbobr.env")?;
    let env_content = env_template
        .replace("{{DOMAIN_REPO}}", &config.domain_repo)
        .replace("{{FORK_OWNER}}", &config.fork_owner);

    let mut files = vec![
        SetupFile {
            path: "README.md".into(),
            content: readme,
        },
        SetupFile {
            path: ".zbobr.env".into(),
            content: env_content,
        },
        SetupFile {
            path: "run.sh".into(),
            content: run_sh,
        },
        SetupFile {
            path: "run.cmd".into(),
            content: run_cmd,
        },
    ];

    // Add all prompts files
    let prompts = [
        "prompts/common.md",
        "prompts/repositories.md",
        "prompts/planner.md",
        "prompts/worker.md",
        "prompts/planner-workflow.md",
        "prompts/worker-workflow.md",
    ];

    for file_name in &prompts {
        let content = load_resource(file_name)?;
        files.push(SetupFile {
            path: file_name.to_string(),
            content,
        });
    }

    Ok(files)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize rustls crypto provider before any TLS operations
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let config = load_config(&cli)?;
    let zbobr = Zbobr::new(config)?;
    let prompts = resolve_prompts(&cli, zbobr.config())?;

    match cli.command {
        Command::Setup {
            dry_run,
            output_dir,
            force,
        } => {
            let files = build_setup_files(&zbobr)?;
            let default_dir = default_setup_dir(&zbobr);
            let dir = output_dir.unwrap_or(default_dir);

            // Stage 1: always write local files
            zbobr.setup_write_local(&dir, &files).await?;

            if dry_run {
                tracing::info!("Dry run: local files written to {}", dir.display());
                tracing::info!("Skipping GitHub push. Run without --dry-run to push.");
            } else {
                // Stage 2: push to GitHub
                zbobr.setup_push_remote(&dir, &files, force).await?;
            }
        }
        Command::Cleanup { dry_run } => {
            zbobr.cleanup_closed_tasks(dry_run).await?;
        }
        Command::Plan { task, model, port } => {
            let prompt = load_prompts(&prompts.planner)?;
            let model_enum = model
                .map(|m| m.parse::<Model>())
                .transpose()
                .map_err(anyhow::Error::msg)?;
            run_role_session(&zbobr, task, Role::Planner, model_enum, port, &prompt).await?;
        }
        Command::Work { task, model, port } => {
            let prompt = load_prompts(&prompts.worker)?;
            let model_enum = model
                .map(|m| m.parse::<Model>())
                .transpose()
                .map_err(anyhow::Error::msg)?;
            run_role_session(&zbobr, task, Role::Worker, model_enum, port, &prompt).await?;
        }
        Command::Loop {
            interval,
            cleanup_interval,
            model,
            port,
            ..
        } => {
            let model_enum = model
                .map(|m| m.parse::<Model>())
                .transpose()
                .map_err(anyhow::Error::msg)?;
            run_manager_loop(
                &zbobr,
                interval,
                cleanup_interval,
                model_enum,
                port,
                cli.global.admin_port,
                &prompts,
            )
            .await?;
        }
    }

    Ok(())
}

/// Start MCP server, invoke CLI tool (copilot/claude/stub), and handle stage transitions.
async fn run_role_session(
    zbobr: &Zbobr,
    task_id: u64,
    role: Role,
    model: Option<Model>,
    port: u16,
    prompt: &str,
) -> anyhow::Result<()> {
    let model = model.unwrap_or_else(|| zbobr.config().default_model.clone());

    // Set stage
    let stage = if role == Role::Planner {
        Stage::Planning
    } else {
        Stage::Working
    };
    zbobr.set_task_stage(task_id, stage).await?;

    // Create workspace dir
    let task_dir = zbobr.config().workspace.join(format!("task#{task_id}"));
    tokio::fs::create_dir_all(&task_dir).await?;

    // Start MCP server in background, scoped to this role and task
    let server_zbobr = zbobr.clone();
    let server_role = role;
    let server_handle = tokio::spawn(async move {
        if let Err(e) =
            zbobr_lib::mcp::run_role_mcp_server(server_zbobr, port, server_role, task_id).await
        {
            tracing::error!("MCP server error: {e}");
        }
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Execute the tool using the ToolExecutor trait
    let cli_tool = zbobr.config().cli_tool;
    let mcp_url = format!("http://127.0.0.1:{port}/{role}/{task_id}");

    let executor = cli_tool.executor();
    if let Err(e) = executor
        .execute(task_id, role, &model, port, prompt, &task_dir, &mcp_url)
        .await
    {
        tracing::error!("Tool execution failed: {e}");
    }

    // On exit, set stage to Pending
    zbobr.set_task_stage(task_id, Stage::Pending).await?;
    tracing::info!("Session complete, task #{task_id} set to PENDING");

    // Shut down server
    server_handle.abort();

    Ok(())
}

/// Main manager loop: polls for tasks and spawns sessions.
async fn run_manager_loop(
    zbobr: &Zbobr,
    interval_secs: u64,
    cleanup_interval_secs: u64,
    model: Option<Model>,
    port: u16,
    admin_port: Option<u16>,
    prompts: &Prompts,
) -> anyhow::Result<()> {
    let model = model.unwrap_or_else(|| zbobr.config().default_model.clone());

    // Load prompts once at loop start
    let planner_prompt = load_prompts(&prompts.planner)?;
    let worker_prompt = load_prompts(&prompts.worker)?;

    tracing::info!("Manager loop started for {}", zbobr.config().domain_repo);
    tracing::info!("Poll interval: {interval_secs}s, Cleanup interval: {cleanup_interval_secs}s");
    tracing::info!("Default Model: {model}");
    tracing::info!("CLI Tool: {:?}", zbobr.config().cli_tool);
    tracing::info!("Planner prompt files: {}", prompts.planner.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("; "));
    tracing::info!("Worker prompt files: {}", prompts.worker.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("; "));
    tracing::info!("Backend: {:?}", zbobr.config().backend);

    let mut last_cleanup = std::time::Instant::now();

    // Start Admin MCP server if port is provided
    if let Some(a_port) = admin_port {
        let admin_zbobr = zbobr.clone();
        tokio::spawn(async move {
            tracing::info!("Starting Admin MCP on port {a_port}");
            if let Err(e) =
                zbobr_lib::mcp::run_admin_mcp_server(admin_zbobr, a_port).await
            {
                tracing::error!("Admin MCP server error: {e}");
            }
        });
    }

    loop {
        // Run cleanup if interval has passed
        if last_cleanup.elapsed().as_secs() >= cleanup_interval_secs {
            tracing::info!("Running workspace cleanup...");
            if let Err(e) = zbobr.cleanup_closed_tasks(false).await {
                tracing::warn!("Cleanup failed: {e}");
            }
            last_cleanup = std::time::Instant::now();
        }

        // Check for processable tasks using tool-based filtering
        let current_tool = zbobr.config().cli_tool;

        // Check for PLANNING_READY tasks
        let planning_tasks = match zbobr
            .list_tasks_by_stage(Stage::PlanningReady.milestone_name(), Some(current_tool))
            .await
        {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::error!("Failed to check PLANNING_READY tasks: {e}");
                vec![]
            }
        };

        if let Some(task) = planning_tasks.first() {
            let task_model = task.model.clone().unwrap_or_else(|| model.clone());
            tracing::info!(
                "Found PLANNING_READY task #{} (tool: {:?}) - running planner",
                task.id,
                task.tool
            );
            if let Err(e) = run_role_session(
                zbobr,
                task.id,
                Role::Planner,
                Some(task_model),
                port,
                &planner_prompt,
            )
            .await
            {
                tracing::error!("Planner session failed: {e}");
            }
            continue;
        }

        // Check for WORKING_READY tasks
        let working_tasks = match zbobr
            .list_tasks_by_stage(Stage::WorkingReady.milestone_name(), Some(current_tool))
            .await
        {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::error!("Failed to check WORKING_READY tasks: {e}");
                vec![]
            }
        };

        if let Some(task) = working_tasks.first() {
            let task_model = task.model.clone().unwrap_or_else(|| model.clone());
            tracing::info!(
                "Found WORKING_READY task #{} (tool: {:?}) - running worker",
                task.id,
                task.tool
            );
            if let Err(e) = run_role_session(
                zbobr,
                task.id,
                Role::Worker,
                Some(task_model),
                port,
                &worker_prompt,
            )
            .await
            {
                tracing::error!("Worker session failed: {e}");
            }
            continue;
        }

        // Log task statistics before sleeping
        tracing::info!(
            "Task statistics for tool {:?}: PLANNING_READY={}, WORKING_READY={}",
            current_tool,
            planning_tasks.len(),
            working_tasks.len()
        );

        if !planning_tasks.is_empty() {
            let summary: Vec<_> = planning_tasks.iter().map(|t| format!("#{} (tool: {:?})", t.id, t.tool)).collect();
            tracing::info!("  PLANNING_READY tasks: {}", summary.join(", "));
        }

        if !working_tasks.is_empty() {
            let summary: Vec<_> = working_tasks.iter().map(|t| format!("#{} (tool: {:?})", t.id, t.tool)).collect();
            tracing::info!("  WORKING_READY tasks: {}", summary.join(", "));
        }

        tracing::info!("No processable tasks. Sleeping {interval_secs}s...");
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
    }
}

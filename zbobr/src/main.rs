#![allow(clippy::needless_borrows_for_generic_args)]
use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, CommandFactory, Parser, Subcommand};
use zbobr_lib::{
    task::{Model, Role, Tool},
    Stage, TomlConfig, Zbobr, ZbobrConfig,
};

#[derive(Args, Clone)]
#[command(next_help_heading = "Global Options")]
struct GlobalArgs {
    /// Domain repository with tasks to orchestrate, in "owner/repo" format
    /// (e.g. "YoroolGui/copilot-zenoh"). Can also be set via ZBOBR_DOMAIN_REPO env var
    #[arg(long)]
    domain_repo: Option<String>,

    /// GitHub user or organization where target repos are forked for implementation
    /// (e.g. "YoroolGui"). Workers fork repos here to create PRs.
    /// Can also be set via ZBOBR_FORK_OWNER env var
    #[arg(long)]
    fork_owner: Option<String>,

    /// Path to workspace directory (default: ./workspace)
    /// Can also be set via ZBOBR_WORKSPACE env var
    #[arg(long)]
    workspace: Option<PathBuf>,

    /// Path to TOML configuration file (default: zbobr.toml in cwd)
    #[arg(long, env = "ZBOBR_CONFIG")]
    config: Option<PathBuf>,

    /// Base directory for prompt files (prompt paths are resolved relative to this)
    /// Can also be set via ZBOBR_PROMPTS_PATH env var
    #[arg(long, env = "ZBOBR_PROMPTS_PATH")]
    prompts_path: Option<PathBuf>,

    /// Semicolon-separated list of prompt files for planner role
    #[arg(long, env = "ZBOBR_PLANNER_PROMPTS", value_delimiter = ';')]
    planner_prompts: Option<Vec<PathBuf>>,

    /// Semicolon-separated list of prompt files for worker role
    #[arg(long, env = "ZBOBR_WORKER_PROMPTS", value_delimiter = ';')]
    worker_prompts: Option<Vec<PathBuf>>,

    /// Backend to use: "github" (default) or "stub"
    #[arg(long, env = "ZBOBR_BACKEND")]
    backend: Option<String>,

    /// CLI tool to use: "copilot", "claude", or "stub"
    #[arg(long, env = "ZBOBR_CLI_TOOL")]
    cli_tool: Option<String>,

    /// Port for the Admin MCP server (optional)
    #[arg(long, env = "ZBOBR_ADMIN_PORT")]
    admin_port: Option<u16>,
}

#[derive(Parser)]
#[command(
    name = "zbobr",
    about = "AI-powered task orchestrator",
    long_about = "AI-powered task orchestrator that manages tasks through automated stages.\n\n\
        Tasks flow through: PENDING -> GO_PLANNING -> PLANNING -> GO_WORKING -> WORKING.\n\
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
    /// Initialize a domain project: create repo if needed, set up stages and labels
    Setup {
        /// Force overwrite existing labels
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
        /// Show the prompt that would be sent to the model instead of running
        #[arg(long)]
        show_prompt: bool,
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
        /// Show the prompt that would be sent to the model instead of running
        #[arg(long)]
        show_prompt: bool,
    },
}

/// Resolved prompt file paths for planner and worker.
struct Prompts {
    base_path: Option<PathBuf>,
    planner: Vec<PathBuf>,
    worker: Vec<PathBuf>,
}

/// Resolve prompt paths: CLI arg > config values.
/// Paths are resolved relative to prompts_path if provided, otherwise relative to current directory.
fn resolve_prompts(cli: &Cli, config: &ZbobrConfig) -> anyhow::Result<Prompts> {
    // Use CLI args if provided, otherwise use config (which came from TOML/env/defaults)
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

    // CLI prompts_path > config.prompts_path (which came from TOML/env)
    let base_path = cli
        .global
        .prompts_path
        .clone()
        .or_else(|| config.prompts_path.clone());

    Ok(Prompts {
        base_path,
        planner,
        worker,
    })
}

/// Load and concatenate multiple prompt files (additional user context).
/// If base_path is provided, relative paths are resolved relative to it.
/// Otherwise, relative paths are resolved relative to the current directory.
/// Missing files are silently skipped (they are optional additional context).
fn load_prompts(paths: &[PathBuf], base_path: Option<&PathBuf>) -> anyhow::Result<String> {
    let mut combined = String::new();
    for path in paths.iter() {
        // Resolve path relative to base_path if provided and path is relative
        let resolved_path = if let Some(base) = base_path {
            if path.is_relative() {
                base.join(path)
            } else {
                path.clone()
            }
        } else if path.is_relative() {
            std::env::current_dir()?.join(path)
        } else {
            path.clone()
        };

        let content = match std::fs::read_to_string(&resolved_path) {
            Ok(c) => c,
            Err(_) => {
                tracing::debug!(
                    "Prompt file not found, skipping: {}",
                    resolved_path.display()
                );
                continue;
            }
        };

        let trimmed = content.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !combined.is_empty() {
            combined.push_str("\n\n");
        }
        combined.push_str(trimmed);
    }
    Ok(combined)
}

/// Build full prompt: hardcoded instructions + user context files + auto-generated API docs.
fn build_full_prompt(user_context: &str, role: Role) -> String {
    let hardcoded = match role {
        Role::Planner => zbobr_lib::planner_instructions(),
        Role::Worker => zbobr_lib::worker_instructions(),
    };

    let api_docs = match role {
        Role::Planner => zbobr_lib::PlannerMcp::generate_api_docs(),
        Role::Worker => zbobr_lib::WorkerMcp::generate_api_docs(),
    };

    if user_context.is_empty() {
        format!("{}\n\n---\n\n{}", hardcoded, api_docs)
    } else {
        format!(
            "{}\n\n---\n\n{}\n\n---\n\n{}",
            hardcoded, user_context, api_docs
        )
    }
}

/// Load TOML config based on CLI args.
/// If --config is specified, load that file (error if missing).
/// Otherwise, try zbobr.toml in cwd (silently skip if missing).
fn load_toml_config(cli: &Cli) -> anyhow::Result<Option<TomlConfig>> {
    if let Some(ref path) = cli.global.config {
        // Explicit --config: must exist
        let config = TomlConfig::load(path)?
            .ok_or_else(|| anyhow::anyhow!("Config file not found: {}", path.display()))?;
        Ok(Some(config))
    } else {
        // Try zbobr.toml in cwd
        let default_path = std::env::current_dir()?.join("zbobr.toml");
        Ok(TomlConfig::load(&default_path)?)
    }
}

fn load_config(cli: &Cli) -> anyhow::Result<ZbobrConfig> {
    let toml_config = load_toml_config(cli)?;
    let mut config = ZbobrConfig::build(toml_config.as_ref())?;

    // CLI arg overrides (highest priority)
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
        config.backend = b
            .parse::<zbobr_lib::config::BackendType>()
            .map_err(|e| anyhow::anyhow!(e))?;
    }
    if let Some(ref t) = cli.global.cli_tool {
        config.cli_tool = t.parse::<Tool>().map_err(|e| anyhow::anyhow!(e))?;
    }
    config.validate()?;
    Ok(config)
}


/// Parse CLI, allowing global options both before and after the subcommand.
///
/// Global options are defined without `global = true` so they only appear in
/// `zbobr --help`, not in subcommand help. To still accept them after the
/// subcommand (e.g. `zbobr setup --config foo.toml`), we reorder raw args
/// to move recognized global flags before the subcommand before parsing.
fn parse_cli() -> Cli {
    let cmd = Cli::command();

    // Collect known subcommand names
    let subcommands: std::collections::HashSet<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_owned())
        .collect();

    // Collect global arg long names (with -- prefix) and whether they take a value
    let global_tmp = GlobalArgs::augment_args(clap::Command::new(""));
    let global_flags: std::collections::HashMap<String, bool> = global_tmp
        .get_arguments()
        .filter_map(|a| {
            a.get_long().map(|long| {
                let takes_value = !matches!(
                    a.get_action(),
                    clap::ArgAction::SetTrue | clap::ArgAction::SetFalse | clap::ArgAction::Count
                );
                (format!("--{long}"), takes_value)
            })
        })
        .collect();

    let raw_args: Vec<String> = std::env::args().collect();
    let mut before_sub = vec![raw_args[0].clone()]; // binary name
    let mut sub_and_after: Vec<String> = Vec::new();
    let mut found_sub = false;

    let mut i = 1;
    while i < raw_args.len() {
        let arg = &raw_args[i];

        if !found_sub {
            if subcommands.contains(arg.as_str()) {
                found_sub = true;
                sub_and_after.push(arg.clone());
            } else {
                before_sub.push(arg.clone());
            }
            i += 1;
            continue;
        }

        // After subcommand: check if this is a global flag to hoist
        // Handle --flag=value syntax
        let base = arg.split('=').next().unwrap_or(arg);
        if let Some(&takes_value) = global_flags.get(base) {
            if arg.contains('=') {
                // --flag=value: move entire arg
                before_sub.push(arg.clone());
                i += 1;
            } else if takes_value && i + 1 < raw_args.len() {
                // --flag value: move both
                before_sub.push(arg.clone());
                before_sub.push(raw_args[i + 1].clone());
                i += 2;
            } else {
                // boolean flag or no value
                before_sub.push(arg.clone());
                i += 1;
            }
        } else {
            sub_and_after.push(arg.clone());
            i += 1;
        }
    }

    before_sub.extend(sub_and_after);
    Cli::parse_from(before_sub)
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

    let cli = parse_cli();
    let config = load_config(&cli)?;
    let zbobr = Zbobr::new(config)?;
    zbobr.validate_connectivity().await?;
    let prompts = resolve_prompts(&cli, zbobr.config())?;

    match cli.command {
        Command::Setup { force } => {
            zbobr.setup(force).await?;
        }
        Command::Cleanup { dry_run } => {
            zbobr.cleanup_closed_tasks(dry_run).await?;
        }
        Command::Plan {
            task,
            model,
            port,
            show_prompt,
        } => {
            let base_prompt = load_prompts(&prompts.planner, prompts.base_path.as_ref())?;
            let full_prompt = build_full_prompt(&base_prompt, Role::Planner);

            if show_prompt {
                println!("{}", full_prompt);
                return Ok(());
            }

            let model_enum = model
                .map(|m| m.parse::<Model>())
                .transpose()
                .map_err(anyhow::Error::msg)?;
            run_role_session(&zbobr, task, Role::Planner, model_enum, port, &full_prompt).await?;
        }
        Command::Work {
            task,
            model,
            port,
            show_prompt,
        } => {
            let base_prompt = load_prompts(&prompts.worker, prompts.base_path.as_ref())?;
            let full_prompt = build_full_prompt(&base_prompt, Role::Worker);

            if show_prompt {
                println!("{}", full_prompt);
                return Ok(());
            }

            let model_enum = model
                .map(|m| m.parse::<Model>())
                .transpose()
                .map_err(anyhow::Error::msg)?;
            run_role_session(&zbobr, task, Role::Worker, model_enum, port, &full_prompt).await?;
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
    base_port: u16,
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

    // Create a channel to receive the actual port from the MCP server
    let (port_tx, port_rx) = std::sync::mpsc::channel();

    // Start MCP server in background, scoped to this role and task
    let server_zbobr = zbobr.clone();
    let server_role = role;
    let server_handle = tokio::spawn(async move {
        match zbobr_lib::mcp::run_role_mcp_server(server_zbobr, base_port, server_role, task_id)
            .await
        {
            Ok(assigned_port) => {
                let _ = port_tx.send(assigned_port);
                tracing::info!("MCP server assigned port {assigned_port}");
            }
            Err(e) => {
                tracing::error!("MCP server error: {e}");
            }
        }
    });

    // Receive the assigned port from the server task
    let assigned_port = port_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .context("MCP server failed to report assigned port in time")?;

    // Execute the tool using the ToolExecutor trait with Ctrl+C handling
    let cli_tool = zbobr.config().cli_tool;
    let mcp_url = format!("http://127.0.0.1:{assigned_port}/{role}/{task_id}");

    let executor = cli_tool.executor();
    let agent_token = &zbobr.config().agent_github_token;
    let copilot_token = &zbobr.config().copilot_github_token;
    let execution_result = tokio::select! {
        result = executor.execute(task_id, role, &model, assigned_port, prompt, &task_dir, &mcp_url, agent_token, copilot_token) => {
            if let Err(e) = result {
                tracing::error!("Tool execution failed: {e}");
            }
            false // Normal completion
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::warn!("Received shutdown signal during execution");
            true // Interrupted
        }
    };

    // Set stage based on signal (refetch task to get updated signal)
    let task = zbobr.get_task(task_id).await?;
    let final_stage = if let Some(signal) = task.signal {
        // Map signal to target stage
        signal.target_stage()
    } else {
        // No signal set, transition to next stage based on role
        if role == Role::Planner {
            Stage::GoWorking
        } else {
            Stage::Pending
        }
    };
    
    zbobr.set_task_stage(task_id, final_stage).await?;
    if execution_result {
        tracing::info!("Session interrupted, task #{task_id} set to {:?}", final_stage);
    } else {
        tracing::info!("Session complete, task #{task_id} set to {:?}", final_stage);
    }

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

    // Load prompts once at loop start and append API docs
    let planner_base = load_prompts(&prompts.planner, prompts.base_path.as_ref())?;
    let worker_base = load_prompts(&prompts.worker, prompts.base_path.as_ref())?;
    let planner_prompt = build_full_prompt(&planner_base, Role::Planner);
    let worker_prompt = build_full_prompt(&worker_base, Role::Worker);

    tracing::info!("Manager loop started for {}", zbobr.config().domain_repo);
    tracing::info!("Poll interval: {interval_secs}s, Cleanup interval: {cleanup_interval_secs}s");
    tracing::info!("Default Model: {model}");
    tracing::info!("CLI Tool: {:?}", zbobr.config().cli_tool);
    if let Some(ref base) = prompts.base_path {
        tracing::info!("Prompts base path: {}", base.display());
    }
    tracing::info!(
        "Planner prompt files: {}",
        prompts
            .planner
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("; ")
    );
    tracing::info!(
        "Worker prompt files: {}",
        prompts
            .worker
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("; ")
    );
    tracing::info!("Backend: {:?}", zbobr.config().backend);

    let mut last_cleanup = std::time::Instant::now();

    // Start Admin MCP server if port is provided
    if let Some(a_port) = admin_port {
        let admin_zbobr = zbobr.clone();
        tokio::spawn(async move {
            tracing::info!("Starting Admin MCP on port {a_port}");
            if let Err(e) = zbobr_lib::mcp::run_admin_mcp_server(admin_zbobr, a_port).await {
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

        // First, check PENDING tasks for signals and transition them.
        // Note: Only PENDING tasks are checked - tasks already in GO_PLANNING or GO_WORKING
        // stages are locked and should not be transitioned by this logic.
        let pending_tasks = match zbobr
            .list_tasks_by_stage(Stage::Pending.milestone_name(), Some(current_tool))
            .await
        {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::error!("Failed to check PENDING tasks: {e}");
                vec![]
            }
        };

        for task in pending_tasks {
            if let Some(signal) = task.signal {
                let target_stage = signal.target_stage();
                if target_stage != Stage::Pending {
                    tracing::info!(
                        "Task #{} has signal {:?}, transitioning from PENDING to {}",
                        task.id,
                        signal,
                        target_stage
                    );
                    if let Err(e) = zbobr.set_task_stage(task.id, target_stage).await {
                        tracing::error!("Failed to transition task #{}: {e}", task.id);
                    }
                }
            }
        }

        // Check for GO_PLANNING tasks
        let planning_tasks = match zbobr
            .list_tasks_by_stage(Stage::GoPlanning.milestone_name(), Some(current_tool))
            .await
        {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::error!("Failed to check GO_PLANNING tasks: {e}");
                vec![]
            }
        };

        if let Some(task) = planning_tasks.first() {
            let task_model = task.model.clone().unwrap_or_else(|| model.clone());
            tracing::info!(
                "Found GO_PLANNING task #{} (tool: {:?}, model: {}) - running planner",
                task.id,
                task.tool,
                task_model
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

        // Check for GO_WORKING tasks
        let working_tasks = match zbobr
            .list_tasks_by_stage(Stage::GoWorking.milestone_name(), Some(current_tool))
            .await
        {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::error!("Failed to check GO_WORKING tasks: {e}");
                vec![]
            }
        };

        if let Some(task) = working_tasks.first() {
            let task_model = task.model.clone().unwrap_or_else(|| model.clone());
            tracing::info!(
                "Found GO_WORKING task #{} (tool: {:?}, model: {}) - running worker",
                task.id,
                task.tool,
                task_model
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
            "Task statistics for tool {:?}: GO_PLANNING={}, GO_WORKING={}",
            current_tool,
            planning_tasks.len(),
            working_tasks.len()
        );

        if !planning_tasks.is_empty() {
            let summary: Vec<_> = planning_tasks
                .iter()
                .map(|t| format!("#{} (tool: {:?}, model: {:?})", t.id, t.tool, t.model))
                .collect();
            tracing::info!("  GO_PLANNING tasks: {}", summary.join(", "));
        }

        if !working_tasks.is_empty() {
            let summary: Vec<_> = working_tasks
                .iter()
                .map(|t| format!("#{} (tool: {:?}, model: {:?})", t.id, t.tool, t.model))
                .collect();
            tracing::info!("  GO_WORKING tasks: {}", summary.join(", "));
        }

        tracing::info!("No processable tasks. Sleeping {interval_secs}s...");

        // Sleep with Ctrl+C handling
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(interval_secs)) => {
                // Continue to next iteration
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received shutdown signal, exiting...");
                break;
            }
        }
    }

    tracing::info!("Manager loop terminated gracefully");
    Ok(())
}

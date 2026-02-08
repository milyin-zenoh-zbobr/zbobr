mod mcp;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use zbobr_lib::{Zbobr, ZbobrConfig, SetupFile, Stage};

#[derive(Args, Clone)]
struct GlobalArgs {
    /// Domain project repo (overrides ZBOBR_DOMAIN_REPO)
    #[arg(long, global = true)]
    domain_repo: Option<String>,

    /// Fork owner (overrides ZBOBR_FORK_OWNER)
    #[arg(long, global = true)]
    fork_owner: Option<String>,

    /// Path to planner prompt file (overrides ZBOBR_PLANNER_PROMPT)
    #[arg(long, env = "ZBOBR_PLANNER_PROMPT", global = true)]
    planner_prompt: Option<PathBuf>,

    /// Path to worker prompt file (overrides ZBOBR_WORKER_PROMPT)
    #[arg(long, env = "ZBOBR_WORKER_PROMPT", global = true)]
    worker_prompt: Option<PathBuf>,
}

#[derive(Parser)]
#[command(name = "zbobr", about = "AI-powered issue orchestrator")]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Set up domain project: create local files, push to GitHub
    Setup {
        /// Only create local directory, do not push to GitHub
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Output directory for local setup files (default: ./<repo-name>)
        #[arg(long, short = 'o')]
        output_dir: Option<PathBuf>,
    },
    /// Run the manager loop
    Loop {
        /// Poll interval in seconds
        #[arg(long, default_value = "60")]
        interval: u64,
        /// Cleanup interval in seconds
        #[arg(long, default_value = "600")]
        cleanup_interval: u64,
        /// AI model to use
        #[arg(long)]
        model: Option<String>,
        /// MCP server port
        #[arg(long, default_value = "3000")]
        port: u16,
    },
    /// Clean up workspace directories for closed issues
    Cleanup {
        #[arg(long, short = 'n')]
        dry_run: bool,
    },
    /// Run planner for a specific issue
    Plan {
        /// Issue number
        issue: u64,
        /// AI model to use
        #[arg(long)]
        model: Option<String>,
        /// MCP server port
        #[arg(long, default_value = "3000")]
        port: u16,
    },
    /// Run worker for a specific issue
    Work {
        /// Issue number
        issue: u64,
        /// AI model to use
        #[arg(long)]
        model: Option<String>,
        /// MCP server port
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

/// Resolved prompt file paths for planner and worker.
struct Prompts {
    planner: PathBuf,
    worker: PathBuf,
}

/// Resolve prompt paths: CLI arg > env var > default (prompts/ next to executable).
fn resolve_prompts(cli: &Cli) -> anyhow::Result<Prompts> {
    let prompts_dir = default_prompts_dir()?;

    let planner = cli
        .global
        .planner_prompt
        .clone()
        .unwrap_or_else(|| prompts_dir.join("planner.md"));

    let worker = cli
        .global
        .worker_prompt
        .clone()
        .unwrap_or_else(|| prompts_dir.join("worker.md"));

    Ok(Prompts { planner, worker })
}

/// Default prompts directory: `prompts/` next to the executable.
fn default_prompts_dir() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine executable directory"))?;
    Ok(exe_dir.join("prompts"))
}

/// Load a prompt from file.
fn load_prompt(path: &PathBuf) -> anyhow::Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read prompt file {}: {e}", path.display()))
}

fn load_config(cli: &Cli) -> Result<ZbobrConfig, zbobr_lib::ZbobrError> {
    let mut config = ZbobrConfig::from_env()?;
    if let Some(ref dr) = cli.global.domain_repo {
        config.domain_repo = dr.clone();
    }
    if let Some(ref fo) = cli.global.fork_owner {
        config.fork_owner = fo.clone();
    }
    Ok(config)
}

/// Default output directory for setup: `./<repo-name>` in the current dir.
fn default_setup_dir(zbobr: &Zbobr) -> PathBuf {
    let repo = &zbobr.config().domain_repo;
    // Use the repo part after the slash, e.g. "Org/my-project" -> "my-project"
    let name = repo.split('/').nth(1).unwrap_or(repo);
    PathBuf::from(name)
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
    let repositories = load_resource("repositories.md")?;
    let run_sh = load_resource("run.sh")?;
    let run_cmd = load_resource("run.cmd")?;

    let env_template = load_resource("zbobr.env")?;
    let env_content = env_template
        .replace("{{DOMAIN_REPO}}", &config.domain_repo)
        .replace("{{FORK_OWNER}}", &config.fork_owner);

    Ok(vec![
        SetupFile {
            path: "README.md".into(),
            content: readme,
        },
        SetupFile {
            path: "repositories.md".into(),
            content: repositories,
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
    ])
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let config = load_config(&cli)?;
    let zbobr = Zbobr::new(config)?;
    let prompts = resolve_prompts(&cli)?;

    match cli.command {
        Command::Setup { dry_run, output_dir } => {
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
                zbobr.setup_push_remote(&dir, &files).await?;
            }
        }
        Command::Cleanup { dry_run } => {
            zbobr.cleanup_closed_tasks(dry_run).await?;
        }
        Command::Plan { issue, model, port } => {
            let prompt = load_prompt(&prompts.planner)?;
            run_agent_session(&zbobr, issue, "planner", model, port, &prompt).await?;
        }
        Command::Work { issue, model, port } => {
            let prompt = load_prompt(&prompts.worker)?;
            run_agent_session(&zbobr, issue, "worker", model, port, &prompt).await?;
        }
        Command::Loop {
            interval,
            cleanup_interval,
            model,
            port,
        } => {
            run_manager_loop(&zbobr, interval, cleanup_interval, model, port, &prompts).await?;
        }
    }

    Ok(())
}

/// Start MCP server, invoke copilot, and handle stage transitions.
async fn run_agent_session(
    zbobr: &Zbobr,
    issue: u64,
    role: &str,
    model: Option<String>,
    port: u16,
    prompt: &str,
) -> anyhow::Result<()> {
    let model = model
        .or_else(|| {
            // Try to get model from the task's label
            None // Will fall back to default below
        })
        .unwrap_or_else(|| zbobr.config().default_model.clone());

    // Set stage
    let stage = if role == "planner" {
        Stage::Planning
    } else {
        Stage::Working
    };
    zbobr.set_task_stage(issue, stage).await?;

    // Create workspace dir
    let issue_dir = zbobr.config().workspace.join(format!("issue#{issue}"));
    tokio::fs::create_dir_all(&issue_dir).await?;

    // Start MCP server in background, scoped to this role and issue
    let server_zbobr = zbobr.clone();
    let server_role = role.to_string();
    let server_handle = tokio::spawn(async move {
        if let Err(e) = mcp::run_mcp_server(server_zbobr, port, server_role, issue).await {
            tracing::error!("MCP server error: {e}");
        }
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Build MCP config for copilot
    let mcp_url = format!("http://127.0.0.1:{port}/{role}/{issue}");
    let mcp_config = serde_json::json!({
        "mcpServers": {
            "zbobr": {
                "url": mcp_url
            }
        }
    });
    let mcp_config_str = serde_json::to_string(&mcp_config)?;

    // Write MCP config to temp file
    let config_path = issue_dir.join(".mcp-config.json");
    tokio::fs::write(&config_path, &mcp_config_str).await?;

    tracing::info!("Starting copilot {role} session for issue #{issue}");
    tracing::info!("MCP endpoint: {mcp_url}");

    let status = tokio::process::Command::new("copilot")
        .args([
            "--model", &model,
            "--additional-mcp-config", config_path.to_str().unwrap(),
            "-i", prompt,
        ])
        .current_dir(&issue_dir)
        .status()
        .await?;

    if !status.success() {
        tracing::warn!("Copilot exited with status: {status}");
    }

    // On copilot exit, set stage to Pending
    zbobr.set_task_stage(issue, Stage::Pending).await?;
    tracing::info!("Session complete, issue #{issue} set to PENDING");

    // Shut down server
    server_handle.abort();

    Ok(())
}

/// Main manager loop: polls for tasks and spawns sessions.
async fn run_manager_loop(
    zbobr: &Zbobr,
    interval_secs: u64,
    cleanup_interval_secs: u64,
    model: Option<String>,
    port: u16,
    prompts: &Prompts,
) -> anyhow::Result<()> {
    let model = model.unwrap_or_else(|| zbobr.config().default_model.clone());

    // Load prompts once at loop start
    let planner_prompt = load_prompt(&prompts.planner)?;
    let worker_prompt = load_prompt(&prompts.worker)?;

    tracing::info!("Manager loop started for {}", zbobr.config().domain_repo);
    tracing::info!("Poll interval: {interval_secs}s, Cleanup interval: {cleanup_interval_secs}s");
    tracing::info!("Model: {model}");
    tracing::info!("Planner prompt: {}", prompts.planner.display());
    tracing::info!("Worker prompt: {}", prompts.worker.display());

    let mut last_cleanup = std::time::Instant::now();

    loop {
        // Run cleanup if interval has passed
        if last_cleanup.elapsed().as_secs() >= cleanup_interval_secs {
            tracing::info!("Running workspace cleanup...");
            if let Err(e) = zbobr.cleanup_closed_tasks(false).await {
                tracing::warn!("Cleanup failed: {e}");
            }
            last_cleanup = std::time::Instant::now();
        }

        // Check for PLANNING issues
        let planning = zbobr.find_tasks_by_stage(Stage::Planning).await?;
        if let Some(task) = planning.first() {
            let task_model = task
                .model
                .clone()
                .unwrap_or_else(|| model.clone());
            tracing::info!("Found PLANNING issue #{} - running planner", task.id);
            run_single_session(zbobr, task.id, "planner", &task_model, port, &planner_prompt)
                .await;
            continue;
        }

        // Check for READY issues
        let ready = zbobr.find_tasks_by_stage(Stage::Ready).await?;
        if let Some(task) = ready.first() {
            let task_model = task
                .model
                .clone()
                .unwrap_or_else(|| model.clone());
            tracing::info!("Found READY issue #{} - running worker", task.id);
            run_single_session(zbobr, task.id, "worker", &task_model, port, &worker_prompt).await;
            continue;
        }

        tracing::info!("No processable issues. Sleeping {interval_secs}s...");
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
    }
}

/// Run a single copilot session (used by manager loop).
/// Starts its own MCP server scoped to the role and issue, then shuts it down after.
async fn run_single_session(
    zbobr: &Zbobr,
    issue: u64,
    role: &str,
    model: &str,
    port: u16,
    prompt: &str,
) {
    // Set stage
    let stage = if role == "planner" {
        Stage::Planning
    } else {
        Stage::Working
    };
    if let Err(e) = zbobr.set_task_stage(issue, stage).await {
        tracing::error!("Failed to set stage for issue #{issue}: {e}");
        return;
    }

    let issue_dir = zbobr.config().workspace.join(format!("issue#{issue}"));
    if let Err(e) = tokio::fs::create_dir_all(&issue_dir).await {
        tracing::error!("Failed to create workspace for issue #{issue}: {e}");
        return;
    }

    // Start MCP server scoped to this role and issue
    let server_zbobr = zbobr.clone();
    let server_role = role.to_string();
    let server_handle = tokio::spawn(async move {
        if let Err(e) = mcp::run_mcp_server(server_zbobr, port, server_role, issue).await {
            tracing::error!("MCP server error: {e}");
        }
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mcp_url = format!("http://127.0.0.1:{port}/{role}/{issue}");
    let mcp_config = serde_json::json!({
        "mcpServers": {
            "zbobr": {
                "url": mcp_url
            }
        }
    });

    let config_path = issue_dir.join(".mcp-config.json");
    if let Err(e) = tokio::fs::write(
        &config_path,
        serde_json::to_string(&mcp_config).unwrap(),
    )
    .await
    {
        tracing::error!("Failed to write MCP config: {e}");
        server_handle.abort();
        return;
    }

    let result = tokio::process::Command::new("copilot")
        .args([
            "--model", model,
            "--additional-mcp-config", config_path.to_str().unwrap(),
            "-i", prompt,
        ])
        .current_dir(&issue_dir)
        .status()
        .await;

    match result {
        Ok(status) if !status.success() => {
            tracing::warn!("Copilot {role} exited with status: {status} for issue #{issue}");
        }
        Err(e) => {
            tracing::error!("Failed to run copilot for issue #{issue}: {e}");
        }
        _ => {}
    }

    // On exit, set stage to Pending
    if let Err(e) = zbobr.set_task_stage(issue, Stage::Pending).await {
        tracing::error!("Failed to set PENDING for issue #{issue}: {e}");
    }

    // Shut down server
    server_handle.abort();

    tracing::info!("Session complete for issue #{issue}");
}

mod mcp;

use clap::{Parser, Subcommand};
use zbobr_lib::{Zbobr, ZbobrConfig, Stage};

#[derive(Parser)]
#[command(name = "zbobr", about = "AI-powered issue orchestrator")]
struct Cli {
    /// Domain project repo (overrides ZBOBR_DOMAIN_REPO)
    #[arg(long)]
    domain_repo: Option<String>,

    /// Fork owner (overrides ZBOBR_FORK_OWNER)
    #[arg(long)]
    fork_owner: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Set up domain project labels and milestones
    Setup {
        #[arg(long, short = 'n')]
        dry_run: bool,
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

fn load_config(cli: &Cli) -> Result<ZbobrConfig, zbobr_lib::ZbobrError> {
    let mut config = ZbobrConfig::from_env()?;
    if let Some(ref dr) = cli.domain_repo {
        config.domain_repo = dr.clone();
    }
    if let Some(ref fo) = cli.fork_owner {
        config.fork_owner = fo.clone();
    }
    Ok(config)
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

    match cli.command {
        Command::Setup { dry_run } => {
            zbobr.setup_domain_project(dry_run).await?;
        }
        Command::Cleanup { dry_run } => {
            zbobr.cleanup_closed_tasks(dry_run).await?;
        }
        Command::Plan { issue, model, port } => {
            run_agent_session(&zbobr, issue, "planner", model, port).await?;
        }
        Command::Work { issue, model, port } => {
            run_agent_session(&zbobr, issue, "worker", model, port).await?;
        }
        Command::Loop {
            interval,
            cleanup_interval,
            model,
            port,
        } => {
            run_manager_loop(&zbobr, interval, cleanup_interval, model, port).await?;
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

    // Start MCP server in background
    let server_zbobr = zbobr.clone();
    let server_handle = tokio::spawn(async move {
        if let Err(e) = mcp::run_mcp_server(server_zbobr, port).await {
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

    // Invoke copilot
    let prompt = if role == "planner" {
        format!("Investigate the task and create an implementation plan. Use the MCP tools to read the plan, update it, and post messages.")
    } else {
        format!("Implement the task according to the plan. Use the MCP tools to get the plan, request repos, submit work, and mark done when finished.")
    };

    let status = tokio::process::Command::new("copilot")
        .args([
            "--model", &model,
            "--additional-mcp-config", config_path.to_str().unwrap(),
            "-i", &prompt,
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
) -> anyhow::Result<()> {
    let model = model.unwrap_or_else(|| zbobr.config().default_model.clone());

    tracing::info!("Manager loop started for {}", zbobr.config().domain_repo);
    tracing::info!("Poll interval: {interval_secs}s, Cleanup interval: {cleanup_interval_secs}s");
    tracing::info!("Model: {model}");

    let mut last_cleanup = std::time::Instant::now();

    // Start MCP server in background for the entire loop
    let server_zbobr = zbobr.clone();
    let _server_handle = tokio::spawn(async move {
        if let Err(e) = mcp::run_mcp_server(server_zbobr, port).await {
            tracing::error!("MCP server error: {e}");
        }
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

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
            run_single_session(zbobr, task.id, "planner", &task_model, port).await;
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
            run_single_session(zbobr, task.id, "worker", &task_model, port).await;
            continue;
        }

        tracing::info!("No processable issues. Sleeping {interval_secs}s...");
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
    }
}

/// Run a single copilot session (used by manager loop).
async fn run_single_session(zbobr: &Zbobr, issue: u64, role: &str, model: &str, port: u16) {
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

    let mcp_url = format!("http://127.0.0.1:{port}/{role}/{issue}");
    let mcp_config = serde_json::json!({
        "mcpServers": {
            "zbobr": {
                "url": mcp_url
            }
        }
    });

    let config_path = issue_dir.join(".mcp-config.json");
    if let Err(e) = tokio::fs::write(&config_path, serde_json::to_string(&mcp_config).unwrap()).await {
        tracing::error!("Failed to write MCP config: {e}");
        return;
    }

    let prompt = if role == "planner" {
        "Investigate the task and create an implementation plan. Use the MCP tools to read the plan, update it, and post messages.".to_string()
    } else {
        "Implement the task according to the plan. Use the MCP tools to get the plan, request repos, submit work, and mark done when finished.".to_string()
    };

    let result = tokio::process::Command::new("copilot")
        .args([
            "--model", model,
            "--additional-mcp-config", config_path.to_str().unwrap(),
            "-i", &prompt,
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

    tracing::info!("Session complete for issue #{issue}");
}

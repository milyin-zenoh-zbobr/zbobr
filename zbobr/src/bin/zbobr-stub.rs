use clap::Parser;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "zbobr-stub")]
struct Args {
    /// Role to simulate: "planner" or "worker"
    #[arg(long)]
    role: String,

    /// Task ID to connect to
    #[arg(long)]
    task_id: u64,

    /// URL of the MCP server (e.g. http://127.0.0.1:3000/planner/123)
    #[arg(long)]
    mcp_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    let role = &args.role;
    let task_id = args.task_id;
    let mcp_url = &args.mcp_url;

    tracing::info!("Stub Tool ({role}) starting for task #{task_id}");
    tracing::info!("Connecting to MCP: {mcp_url}");

    let client = reqwest::Client::new();

    // Check MCP server connectivity
    let mut connected = false;
    for _ in 0..5 {
        if let Ok(resp) = client.get(mcp_url).send().await {
            tracing::info!("MCP Server is reachable: {}", resp.status());
            connected = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    if !connected {
        tracing::warn!(
            "Could not reach MCP server at {mcp_url}, proceeding with simulation anyway."
        );
    }

    if role == "planner" {
        tracing::info!("Stub: analyzing requirements...");
        tokio::time::sleep(Duration::from_secs(1)).await;

        tracing::info!("Stub: creating plan...");
        // In a real stub, we would call `set_plan` tool via MCP.
        // For now, we just simulate the delay and logging.
        tokio::time::sleep(Duration::from_secs(1)).await;
    } else {
        tracing::info!("Stub: working on implementation...");
        tokio::time::sleep(Duration::from_secs(1)).await;

        tracing::info!("Stub: creating PR...");
        // In a real stub, we would call `submit_work` tool via MCP.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    tracing::info!("Stub: Work complete.");
    Ok(())
}

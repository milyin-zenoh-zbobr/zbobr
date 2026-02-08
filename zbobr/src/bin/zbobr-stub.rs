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

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::ACCEPT,
                "application/json, text/event-stream".parse().unwrap(),
            );
            headers.insert(
                reqwest::header::CONTENT_TYPE,
                "application/json".parse().unwrap(),
            );
            headers
        })
        .build()?;

    // Check MCP server connectivity
    let mut connected = false;
    for _ in 0..5 {
        if let Ok(resp) = client.get(mcp_url).send().await {
            let status: reqwest::StatusCode = resp.status();
            tracing::info!("MCP Server is reachable: {}", status);
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

    // MCP Handshake
    tracing::info!("Stub: performing MCP handshake...");
    let init_payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "zbobr-stub", "version": "0.1.0" }
        },
        "id": 0
    });
    let _ = client.post(mcp_url).json(&init_payload).send().await;

    let initialized_notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let _ = client
        .post(mcp_url)
        .json(&initialized_notification)
        .send()
        .await;

    if role == "planner" {
        tracing::info!("Stub: analyzing requirements...");
        tokio::time::sleep(Duration::from_secs(1)).await;

        tracing::info!("Stub: creating plan...");
        let plan = format!(
            "Implementation plan for task #{task_id}\n\n1. Add new feature\n2. Fix existing bug"
        );
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "set_plan",
                "arguments": {
                    "plan": plan
                }
            },
            "id": 1
        });

        if let Err(e) = client.post(mcp_url).json(&payload).send().await {
            tracing::error!("Failed to set plan via MCP: {e}");
        } else {
            tracing::info!("Stub: plan set via MCP.");
        }

        let msg_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "post_message",
                "arguments": {
                    "message": "Planner: requirements analyzed and plan created."
                }
            },
            "id": 2
        });
        let _ = client.post(mcp_url).json(&msg_payload).send().await;
    } else {
        tracing::info!("Stub: working on implementation...");
        tokio::time::sleep(Duration::from_secs(1)).await;

        tracing::info!("Stub: creating PR...");
        // Simulation of PR creation via submit_work
        let submit_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "submit_work",
                "arguments": {
                    "repo": "stub/repo"
                }
            },
            "id": 1
        });
        let _ = client.post(mcp_url).json(&submit_payload).send().await;

        tracing::info!("Stub: marking as done...");
        let done_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "mark_done",
                "arguments": {}
            },
            "id": 2
        });
        if let Err(e) = client.post(mcp_url).json(&done_payload).send().await {
            tracing::error!("Failed to mark done via MCP: {e}");
        } else {
            tracing::info!("Stub: task marked as done via MCP.");
        }
    }

    tracing::info!("Stub: Work complete.");
    Ok(())
}

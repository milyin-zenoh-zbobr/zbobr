use std::time::Duration;

use clap::Parser;
use serde::Serialize;
use serde_json::{json, Value};
use zbobr_lib::{
    mcp::{planner_tools, worker_tools, MessageParam},
    task::Role,
};

#[derive(Parser, Debug)]
#[command(name = "zbobr-stub")]
struct Args {
    /// Role to simulate: "planner" or "worker"
    #[arg(long)]
    role: Role,

    /// Task ID to connect to
    #[arg(long)]
    task_id: u64,

    /// URL of the MCP server (e.g. http://127.0.0.1:3000/planner/123)
    #[arg(long)]
    mcp_url: String,
}

struct McpClient {
    client: reqwest::Client,
    url: String,
    session_id: reqwest::header::HeaderValue,
}

impl McpClient {
    async fn new(url: &str) -> anyhow::Result<Self> {
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
        let mut reachable = false;
        for _ in 0..10 {
            if let Ok(_resp) = client.get(url).send().await {
                reachable = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        if !reachable {
            anyhow::bail!("MCP server at {} is not reachable (no response)", url);
        }

        // Handshake
        let init_payload = json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "zbobr-stub", "version": "0.1.0" }
            },
            "id": 0
        });

        let resp = client.post(url).json(&init_payload).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "MCP Handshake failed: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        let session_id = resp
            .headers()
            .get("mcp-session-id")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No mcp-session-id in MCP response"))?;

        // Initialized notification
        client
            .post(url)
            .header("mcp-session-id", &session_id)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }))
            .send()
            .await?;

        Ok(Self {
            client,
            url: url.to_string(),
            session_id,
        })
    }

    async fn call_tool<P: Serialize>(&self, tool: &str, params: P) -> anyhow::Result<Value> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": tool,
                "arguments": params
            },
            "id": 1
        });

        let resp = self
            .client
            .post(&self.url)
            .header("mcp-session-id", &self.session_id)
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Tool call '{}' failed with status: {} - {}",
                tool,
                resp.status(),
                resp.text().await?
            );
        }

        // SSE handling - this is a simplified version for common response bodies
        let text = resp.text().await?;
        for line in text.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix("data:") {
                let data = stripped.trim();
                if !data.is_empty() && data.starts_with('{') {
                    return Ok(serde_json::from_str(data)?);
                }
            }
        }

        anyhow::bail!(
            "No JSON data found in SSE response for tool '{}': {}",
            tool,
            text
        )
    }
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

    let mcp = McpClient::new(mcp_url).await?;
    tracing::info!("MCP Handshake successful (session: {:?})", mcp.session_id);

    if *role == Role::Planner {
        tracing::info!("Stub: analyzing requirements...");
        tokio::time::sleep(Duration::from_secs(1)).await;

        tracing::info!("Stub: creating plan...");
        let plan = format!(
            "Implementation plan for task #{task_id}\n\n1. Add new feature\n2. Fix existing bug"
        );

        mcp.call_tool(
            planner_tools::POST_MESSAGE,
            MessageParam {
                message: format!("## Implementation Plan\n\n{plan}\n\n---\n\nRequirements analyzed and plan created."),
            },
        )
        .await?;
        tracing::info!("Stub: plan posted as message.");
    } else {
        tracing::info!("Stub: working on implementation...");
        tokio::time::sleep(Duration::from_secs(1)).await;

        tracing::info!("Stub: pulling work repository...");
        mcp.call_tool(
            worker_tools::PULL_WORK,
            json!({}),
        )
        .await?;
        tracing::info!("Stub: work repository ready.");

        tokio::time::sleep(Duration::from_secs(1)).await;

        tracing::info!("Stub: pushing work...");
        mcp.call_tool(
            worker_tools::PUSH_WORK,
            json!({}),
        )
        .await?;
        tracing::info!("Stub: work submitted via MCP.");

        tracing::info!("Stub: marking all checklist items as done...");
        // Check all checklist items to trigger auto-done label
        mcp.call_tool(
            worker_tools::CHECK_CHECKLIST_ITEM,
            json!({
                "id": "stub-item-1",
                "checked": true
            }),
        )
        .await?;
        tracing::info!("Stub: checklist item marked as done, done label should be auto-set.");
    }

    tracing::info!("Stub: Work complete.");
    Ok(())
}

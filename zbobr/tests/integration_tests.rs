use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::time::sleep;
use zbobr_lib::{
    mcp::{admin_tools, CreateTaskParam, SetStageParam, TaskIdParam},
    Stage,
};

struct AdminClient {
    client: reqwest::Client,
    url: String,
    session_id: reqwest::header::HeaderValue,
}

impl AdminClient {
    #[allow(clippy::single_match)]
    async fn new(url: &str) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::ACCEPT,
                    "application/json, text/event-stream".parse().unwrap(),
                );
                headers
            })
            .build()?;

        let init_payload = json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-admin", "version": "0.1.0" }
            },
            "id": 0
        });

        let mut session_id = None;
        for _ in 0..20 {
            match client.post(url).json(&init_payload).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        if let Some(sid) = resp.headers().get("mcp-session-id") {
                            session_id = Some(sid.clone());
                        }

                        // Send initialized notification
                        let mut req = client.post(url).json(&json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/initialized"
                        }));
                        if let Some(ref sid) = session_id {
                            req = req.header("mcp-session-id", sid);
                        }
                        req.send().await?;
                        break;
                    }
                }
                Err(_) => {}
            }
            sleep(Duration::from_millis(500)).await;
        }

        let session_id =
            session_id.ok_or_else(|| anyhow::anyhow!("Admin MCP did not become ready"))?;

        Ok(Self {
            client,
            url: url.to_string(),
            session_id,
        })
    }

    async fn call_tool<P: Serialize>(&self, name: &str, params: P) -> anyhow::Result<Value> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": params
            },
            "id": 1 // Simplified ID for tests
        });

        let resp = self
            .client
            .post(&self.url)
            .header("mcp-session-id", &self.session_id)
            .json(&payload)
            .send()
            .await?;

        let text = resp.text().await?;
        Self::parse_sse(&text)
    }

    fn parse_sse(text: &str) -> anyhow::Result<Value> {
        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = line.trim_start_matches("data: ").trim();
                if !data.is_empty() && data.starts_with('{') {
                    return Ok(serde_json::from_str(data)?);
                }
            }
        }
        anyhow::bail!("No JSON data found in SSE response: {}", text)
    }

    async fn create_task(&self, title: &str, description: &str) -> anyhow::Result<u64> {
        let res = self
            .call_tool(
                admin_tools::CREATE_TASK,
                CreateTaskParam {
                    title: title.to_string(),
                    description: description.to_string(),
                    tool: None,
                    model: None,
                    parent_task_id: None,
                },
            )
            .await?;

        let result_text = res["result"]["content"][0]["text"].as_str().unwrap_or("");
        let id = result_text
            .split('#')
            .nth(1)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| anyhow::anyhow!("Could not extract task ID from: {}", result_text))?;
        Ok(id)
    }

    async fn set_task_stage(&self, id: u64, stage: Stage) -> anyhow::Result<()> {
        self.call_tool(admin_tools::SET_TASK_STAGE, SetStageParam { id, stage })
            .await?;
        Ok(())
    }

    async fn get_task_info(&self, id: u64) -> anyhow::Result<String> {
        let res = self
            .call_tool(admin_tools::GET_TASK, TaskIdParam { id })
            .await?;
        Ok(res["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    async fn get_discussion(&self, id: u64) -> anyhow::Result<String> {
        let res = self
            .call_tool(admin_tools::GET_DISCUSSION, TaskIdParam { id })
            .await?;
        Ok(res["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }
}

#[ignore] // TODO: Re-enable when logic is finalized
#[tokio::test]
async fn test_blackbox_process_flow() -> anyhow::Result<()> {
    // 1. Setup workspace
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path().to_path_buf();

    // Find zbobr binary
    let exe = std::env::current_exe()?
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("zbobr");

    if !exe.exists() {
        return Err(anyhow::anyhow!("zbobr binary not found at {:?}", exe));
    }

    let admin_port = 3088;
    let agent_port = 3089;

    // 2. Start zbobr loop with admin port
    let mut child = tokio::process::Command::new(&exe)
        .env("GH_TOKEN", "stub-token")
        .env("ZBOBR_AGENT_GH_TOKEN", "stub-agent-token")
        .env("RUST_LOG", "info")
        .args([
            "--admin-port",
            &admin_port.to_string(),
            "loop",
            "--interval",
            "1",
            "--port",
            &agent_port.to_string(),
            "--backend",
            "stub",
            "--cli-tool",
            "stub",
            "--workspace",
            workspace.to_str().unwrap(),
            "--domain-repo",
            "test/domain",
            "--fork-owner",
            "test-forks",
        ])
        .spawn()?;

    println!("Spawned zbobr (pid: {:?}) from {:?}", child.id(), exe);

    let admin = AdminClient::new(&format!("http://127.0.0.1:{admin_port}/admin")).await?;

    // 1. Create a task via Admin MCP
    let task_id = admin
        .create_task("Blackbox Feature", "Description of blackbox feature")
        .await?;
    println!("Extracted task ID: {}", task_id);

    // 2. Set stage to GO_PLANNING
    println!("Transitioning to GO_PLANNING...");
    admin.set_task_stage(task_id, Stage::GoPlanning).await?;

    // 3. Wait for planner to post a plan and transition to GO_WORKING
    println!("Waiting for planner to post a plan...");
    let mut plan_ready = false;
    for _ in 0..30 {
        let info = admin.get_task_info(task_id).await?;
        let discussion = admin.get_discussion(task_id).await?;
        let has_plan = discussion.contains("Implementation Plan")
            || discussion.contains("Implementation plan");
        if info.contains("Stage: GoWorking") && has_plan {
            println!("Planner finished, plan is available in discussion.");
            plan_ready = true;
            break;
        }
        sleep(Duration::from_millis(1000)).await;
    }
    assert!(plan_ready, "Planner did not complete successfully");

    // 6. Transition to GO_WORKING
    println!("Transitioning to GO_WORKING...");
    admin.set_task_stage(task_id, Stage::GoWorking).await?;

    // 7. Wait for transition to WORKING
    println!("Waiting for transition to WORKING...");
    let mut done = false;
    for _ in 0..30 {
        let text = admin.get_task_info(task_id).await?;
        if text.contains("Done: true") {
            done = true;
            break;
        }
        sleep(Duration::from_millis(1000)).await;
    }

    assert!(done, "Task did not reach DONE state");
    println!("Blackbox test passed!");

    child.kill().await?;
    Ok(())
}

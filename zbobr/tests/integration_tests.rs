use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;
use zbobr_lib::mcp::{admin_tools, CreateIssueParam, IssueIdParam, SetStageParam};

struct AdminClient {
    client: reqwest::Client,
    url: String,
    session_id: reqwest::header::HeaderValue,
}

impl AdminClient {
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

    async fn create_issue(&self, title: &str, body: &str) -> anyhow::Result<u64> {
        let res = self
            .call_tool(
                admin_tools::CREATE_ISSUE,
                CreateIssueParam {
                    title: title.to_string(),
                    body: body.to_string(),
                },
            )
            .await?;

        let result_text = res["result"]["content"][0]["text"].as_str().unwrap_or("");
        let id = result_text
            .split('#')
            .nth(1)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| anyhow::anyhow!("Could not extract issue ID from: {}", result_text))?;
        Ok(id)
    }

    async fn set_issue_stage(&self, id: u64, stage: &str) -> anyhow::Result<()> {
        self.call_tool(
            admin_tools::SET_ISSUE_STAGE,
            SetStageParam {
                id,
                stage: stage.to_string(),
            },
        )
        .await?;
        Ok(())
    }

    async fn get_issue_info(&self, id: u64) -> anyhow::Result<String> {
        let res = self
            .call_tool(admin_tools::GET_ISSUE, IssueIdParam { id })
            .await?;
        Ok(res["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }
}

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

    let admin_port = 3033;
    let agent_port = 3034;

    // 2. Start zbobr loop with admin port
    let mut child = tokio::process::Command::new(&exe)
        .env("GH_TOKEN", "stub-token")
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
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    println!("Spawned zbobr (pid: {:?}) from {:?}", child.id(), exe);

    let admin = AdminClient::new(&format!("http://127.0.0.1:{admin_port}/admin")).await?;

    // 3. Create an issue via Admin MCP
    let issue_id = admin
        .create_issue("Blackbox Feature", "Description of blackbox feature")
        .await?;
    println!("Extracted issue ID: {}", issue_id);

    // 4. Move to PLANNING_READY
    println!("Transitioning to PLANNING_READY...");
    admin.set_issue_stage(issue_id, "PLANNING_READY").await?;

    // 5. Poll until planner finishes (back to PENDING with a plan)
    let mut plan_ready = false;
    for _ in 0..30 {
        let text = admin.get_issue_info(issue_id).await?;
        if text.contains("stage: Pending") && text.contains("Implementation plan") {
            println!("Planner finished, plan is available.");
            plan_ready = true;
            break;
        }
        sleep(Duration::from_millis(1000)).await;
    }
    assert!(plan_ready, "Planner did not complete successfully");

    // 6. Move to WORKING_READY
    println!("Transitioning to WORKING_READY...");
    admin.set_issue_stage(issue_id, "WORKING_READY").await?;

    // 7. Poll until DONE
    let mut done = false;
    for _ in 0..30 {
        let text = admin.get_issue_info(issue_id).await?;
        if text.contains("done: true") {
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

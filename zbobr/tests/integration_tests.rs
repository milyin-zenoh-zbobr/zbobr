use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

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

    // Helper to call Admin MCP
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
    let admin_url = format!("http://127.0.0.1:{admin_port}/admin");

    // Helper to parse SSE-formatted response
    fn parse_sse(text: &str) -> anyhow::Result<serde_json::Value> {
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

    // Wait for Admin MCP to be ready and capture session ID
    let mut ready = false;
    let mut session_id: Option<reqwest::header::HeaderValue> = None;
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

    for _ in 0..20 {
        match client.post(&admin_url).json(&init_payload).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    if let Some(sid) = resp.headers().get("mcp-session-id") {
                        session_id = Some(sid.clone());
                        println!("Captured session ID: {:?}", session_id);
                    }
                    ready = true;

                    // Send initialized notification
                    let mut req = client
                        .post(&admin_url)
                        .json(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
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
    assert!(ready, "Admin MCP did not become ready at {}", admin_url);
    let session_id = session_id.expect("Did not receive a mcp-session-id from Admin MCP");

    // 3. Create an issue via Admin MCP
    let create_payload = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "create_issue",
            "arguments": {
                "title": "Blackbox Feature",
                "body": "Description of blackbox feature"
            }
        },
        "id": 1
    });

    let resp = client
        .post(&admin_url)
        .header("mcp-session-id", &session_id)
        .json(&create_payload)
        .send()
        .await?;
    let body = parse_sse(&resp.text().await?)?;

    let result_text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    let issue_id = result_text
        .split('#')
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok())
        .expect(&format!("Could not extract issue ID from: {}", result_text));
    println!("Extracted issue ID: {}", issue_id);

    // 4. Move to PLANNING_READY
    println!("Transitioning to PLANNING_READY...");
    let stage_payload = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "set_issue_stage",
            "arguments": {
                "id": issue_id,
                "stage": "PLANNING_READY"
            }
        },
        "id": 2
    });
    let _ = client
        .post(&admin_url)
        .header("mcp-session-id", &session_id)
        .json(&stage_payload)
        .send()
        .await?;

    // 5. Poll until planner finishes (back to PENDING with a plan)
    let mut plan_ready = false;
    for _ in 0..30 {
        let get_payload = json!({
            "jsonrpc": "2.0", "method": "tools/call",
            "params": { "name": "get_issue", "arguments": { "id": issue_id } },
            "id": 10
        });
        let resp = client
            .post(&admin_url)
            .header("mcp-session-id", &session_id)
            .json(&get_payload)
            .send()
            .await?;
        let body = parse_sse(&resp.text().await?)?;
        let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");

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
    let work_payload = json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": { "name": "set_issue_stage", "arguments": { "id": issue_id, "stage": "WORKING_READY" } },
        "id": 11
    });
    let _ = client
        .post(&admin_url)
        .header("mcp-session-id", &session_id)
        .json(&work_payload)
        .send()
        .await?;

    // 7. Poll until DONE
    let mut done = false;
    for _ in 0..30 {
        let get_payload = json!({
            "jsonrpc": "2.0", "method": "tools/call",
            "params": { "name": "get_issue", "arguments": { "id": issue_id } },
            "id": 12
        });
        let resp = client
            .post(&admin_url)
            .header("mcp-session-id", &session_id)
            .json(&get_payload)
            .send()
            .await?;
        let body = parse_sse(&resp.text().await?)?;
        let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");

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

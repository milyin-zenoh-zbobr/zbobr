use crate::task::{Model, Role, Tool};
use async_trait::async_trait;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Trait for executing AI tools with specific configurations.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute the tool for a given task and role.
    ///
    /// # Arguments
    /// * `task_id` - The task ID
    /// * `role` - The role (Planner or Worker)
    /// * `model` - The AI model to use
    /// * `port` - The MCP server port
    /// * `prompt` - The prompt text for the tool
    /// * `task_dir` - The task working directory
    /// * `mcp_url` - The MCP server URL
    async fn execute(
        &self,
        task_id: u64,
        role: Role,
        model: &Model,
        port: u16,
        prompt: &str,
        task_dir: &Path,
        mcp_url: &str,
    ) -> anyhow::Result<()>;
}

/// Executor for GitHub Copilot CLI.
pub struct CopilotExecutor;

#[async_trait]
impl ToolExecutor for CopilotExecutor {
    async fn execute(
        &self,
        task_id: u64,
        role: Role,
        model: &Model,
        _port: u16,
        prompt: &str,
        task_dir: &Path,
        mcp_url: &str,
    ) -> anyhow::Result<()> {
        // Build MCP config for copilot
        let mcp_config = serde_json::json!({
            "mcpServers": {
                "zbobr": {
                    "type": "http",
                    "url": mcp_url,
                    "tools": ["*"]
                }
            }
        });
        let mcp_config_str = serde_json::to_string(&mcp_config)?;

        let model_name = model
            .model_name_for_tool(Tool::Copilot)
            .ok_or_else(|| anyhow::anyhow!("Model {} is not supported by copilot", model))?;

        tracing::info!("Starting copilot {role} session for task #{task_id}");
        tracing::info!("MCP endpoint: {mcp_url}");
        tracing::debug!("MCP config JSON: {}", mcp_config_str);

        let args = [
            "--model",
            model_name,
            "--additional-mcp-config",
            &mcp_config_str,
            "--no-ask-user",
            "--allow-all-tools",
            "--allow-all-urls",
            "-p",
            prompt,
        ];
        tracing::debug!("Copilot command: copilot {}", args.join(" "));

        let mut child = tokio::process::Command::new("copilot")
            .args(args)
            .current_dir(task_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");

        // Spawn tasks to stream stdout and stderr
        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!("[copilot] {}", line);
            }
        });

        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!("[copilot] {}", line);
            }
        });

        // Wait for process to complete
        let status = child.wait().await?;

        // Wait for output tasks to finish
        let _ = tokio::join!(stdout_task, stderr_task);

        tracing::debug!("Copilot finished execution with status: {status}");

        if !status.success() {
            tracing::warn!("copilot exited with status: {status}");
        }

        Ok(())
    }
}

/// Executor for Claude CLI.
pub struct ClaudeExecutor;

#[async_trait]
impl ToolExecutor for ClaudeExecutor {
    async fn execute(
        &self,
        task_id: u64,
        role: Role,
        model: &Model,
        _port: u16,
        prompt: &str,
        task_dir: &Path,
        mcp_url: &str,
    ) -> anyhow::Result<()> {
        // Build MCP config for claude
        let mcp_config = serde_json::json!({
            "mcpServers": {
                "zbobr": {
                    "type": "http",
                    "url": mcp_url,
                    "tools": ["*"]
                }
            }
        });
        let mcp_config_str = serde_json::to_string(&mcp_config)?;

        let model_name = model
            .model_name_for_tool(Tool::Claude)
            .ok_or_else(|| anyhow::anyhow!("Model {} is not supported by claude", model))?;

        tracing::info!("Starting claude {role} session for task #{task_id}");
        tracing::info!("MCP endpoint: {mcp_url}");
        tracing::debug!("MCP config JSON: {}", mcp_config_str);

        let args = [
            "--model",
            model_name,
            "--additional-mcp-config",
            &mcp_config_str,
            "--permission-mode",
            "dontAsk",
            "--tools",
            "default",
            "-p",
            prompt,
        ];
        tracing::debug!("Claude command: claude {}", args.join(" "));

        // Note: Claude CLI execution is not yet fully implemented
        // This is a placeholder for future implementation
        let mut child = tokio::process::Command::new("claude")
            .args(args)
            .current_dir(task_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");

        // Spawn tasks to stream stdout and stderr
        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!("[claude] {}", line);
            }
        });

        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!("[claude] {}", line);
            }
        });

        // Wait for process to complete
        let status = child.wait().await?;

        // Wait for output tasks to finish
        let _ = tokio::join!(stdout_task, stderr_task);

        tracing::debug!("Claude finished execution with status: {status}");

        if !status.success() {
            tracing::warn!("claude exited with status: {status}");
        }

        Ok(())
    }
}

/// Executor for the Stub tool (for testing).
pub struct StubExecutor;

#[async_trait]
impl ToolExecutor for StubExecutor {
    async fn execute(
        &self,
        task_id: u64,
        role: Role,
        _model: &Model,
        _port: u16,
        _prompt: &str,
        _task_dir: &Path,
        mcp_url: &str,
    ) -> anyhow::Result<()> {
        tracing::info!("Running STUB TOOL for {} session", role);

        let exe = std::env::current_exe()?;
        // Assume zbobr-stub is next to zbobr executable
        let stub_exe = exe.parent().unwrap().join("zbobr-stub");

        let mut child = tokio::process::Command::new(stub_exe)
            .args([
                "--role",
                role.as_str(),
                "--task-id",
                &task_id.to_string(),
                "--mcp-url",
                mcp_url,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");

        // Spawn tasks to stream stdout and stderr
        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!("[stub] {}", line);
            }
        });

        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!("[stub] {}", line);
            }
        });

        // Wait for process to complete
        let status = child.wait().await?;

        // Wait for output tasks to finish
        let _ = tokio::join!(stdout_task, stderr_task);

        tracing::debug!("Stub tool finished execution with status: {status}");

        if !status.success() {
            tracing::warn!("Stub tool exited with status: {status}");
        }

        Ok(())
    }
}

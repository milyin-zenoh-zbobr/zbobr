use crate::task::{Model, Tool};
use async_trait::async_trait;
use std::path::Path;

/// Trait for executing AI tools with specific configurations.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute the tool for a given task and role.
    ///
    /// # Arguments
    /// * `task_id` - The task ID
    /// * `role` - The role name ("planner" or "worker")
    /// * `model` - The AI model to use
    /// * `port` - The MCP server port
    /// * `prompt` - The prompt text for the tool
    /// * `task_dir` - The task working directory
    /// * `mcp_url` - The MCP server URL
    async fn execute(
        &self,
        task_id: u64,
        role: &str,
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
        role: &str,
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
                    "url": mcp_url
                }
            }
        });
        let mcp_config_str = serde_json::to_string(&mcp_config)?;

        // Write MCP config to temp file
        let config_path = task_dir.join(".mcp-config.json");
        tokio::fs::write(&config_path, &mcp_config_str).await?;

        let model_name = model
            .model_name_for_tool(Tool::Copilot)
            .ok_or_else(|| anyhow::anyhow!("Model {} is not supported by copilot", model))?;

        tracing::info!("Starting copilot {role} session for task #{task_id}");
        tracing::info!("MCP endpoint: {mcp_url}");

        let status = tokio::process::Command::new("copilot")
            .args([
                "--model",
                model_name,
                "--additional-mcp-config",
                config_path.to_str().unwrap(),
                "-i",
                prompt,
            ])
            .current_dir(task_dir)
            .status()
            .await?;

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
        role: &str,
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
                    "url": mcp_url
                }
            }
        });
        let mcp_config_str = serde_json::to_string(&mcp_config)?;

        // Write MCP config to temp file
        let config_path = task_dir.join(".mcp-config.json");
        tokio::fs::write(&config_path, &mcp_config_str).await?;

        let model_name = model
            .model_name_for_tool(Tool::Claude)
            .ok_or_else(|| anyhow::anyhow!("Model {} is not supported by claude", model))?;

        tracing::info!("Starting claude {role} session for task #{task_id}");
        tracing::info!("MCP endpoint: {mcp_url}");

        // Note: Claude CLI execution is not yet fully implemented
        // This is a placeholder for future implementation
        let status = tokio::process::Command::new("claude")
            .args([
                "--model",
                model_name,
                "--additional-mcp-config",
                config_path.to_str().unwrap(),
                "-i",
                prompt,
            ])
            .current_dir(task_dir)
            .status()
            .await?;

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
        role: &str,
        _model: &Model,
        _port: u16,
        _prompt: &str,
        _task_dir: &Path,
        mcp_url: &str,
    ) -> anyhow::Result<()> {
        tracing::info!("Running STUB TOOL for {role} session");

        let exe = std::env::current_exe()?;
        // Assume zbobr-stub is next to zbobr executable
        let stub_exe = exe.parent().unwrap().join("zbobr-stub");

        let status = tokio::process::Command::new(stub_exe)
            .args([
                "--role",
                role,
                "--task-id",
                &task_id.to_string(),
                "--mcp-url",
                mcp_url,
            ])
            .status()
            .await?;

        if !status.success() {
            tracing::warn!("Stub tool exited with status: {status}");
        }

        Ok(())
    }
}

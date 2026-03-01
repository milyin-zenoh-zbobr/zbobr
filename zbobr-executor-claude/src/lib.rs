use std::{path::Path, process::Stdio};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use zbobr_dispatcher::{
    task::{Model, Role, Tool},
    tool_executor::{ToolExecutor, format_command_for_log},
};

pub mod config;
pub use config::{ZbobrExecutorClaudeArgs, ZbobrExecutorClaudeConfig, ZbobrExecutorClaudeToml};

/// Executor for Claude CLI.
pub struct ClaudeExecutor {
    pub config: ZbobrExecutorClaudeConfig,
}

fn set_github_envs(cmd: &mut tokio::process::Command, agent_github_token: &str, copilot_github_token: &str) {
    tracing::info!("Setting GH_TOKEN for agent and COPILOT_GITHUB_TOKEN for copilot");
    if !agent_github_token.is_empty() {
        cmd.env("GH_TOKEN", agent_github_token)
            .env("GITHUB_TOKEN", agent_github_token);
    } else {
        tracing::debug!("Not setting GH_TOKEN/GITHUB_TOKEN - preserving environment");
    }
    if !copilot_github_token.is_empty() {
        cmd.env("COPILOT_GITHUB_TOKEN", copilot_github_token);
    } else {
        tracing::debug!("Not setting COPILOT_GITHUB_TOKEN - preserving environment");
    }
}

#[async_trait]
impl ToolExecutor for ClaudeExecutor {
    async fn execute(
        &self,
        task_id: u64,
        role: Role,
        model: &Model,
        _port: u16,
        prompt: &str,
        work_dir: &Path,
        mcp_url: &str,
        agent_github_token: &str,
        copilot_github_token: &str,
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

        tracing::info!(
            "Starting claude {role} session for task #{task_id} with model {model_name}"
        );
        tracing::info!("MCP endpoint: {mcp_url}");
        tracing::debug!("MCP config JSON: {}", mcp_config_str);

        let args = [
            "--model",
            model_name,
            "--additional-mcp-config",
            &mcp_config_str,
            "--permission-mode",
            "plan",
            "--tools",
            "default",
            "-p",
            prompt,
        ];

        let mut cmd = tokio::process::Command::new("claude");
        cmd.args(args)
            .current_dir(work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        tracing::debug!(
            "Command: {}",
            format_command_for_log("claude", &args, work_dir)
        );

        // Set GitHub tokens for claude agent process (only set when non-empty to avoid overwriting host env)
        set_github_envs(&mut cmd, agent_github_token, copilot_github_token);

        let mut child = cmd.spawn()?;

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
            let mut collected = Vec::new();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!("[claude] {}", line);
                collected.push(line);
            }
            collected
        });

        // Wait for process to complete
        let status = child.wait().await?;

        // Wait for output tasks to finish
        let (_, stderr_result) = tokio::join!(stdout_task, stderr_task);

        tracing::debug!("Claude finished execution with status: {status}");

        if !status.success() {
            let error_context = match stderr_result {
                Ok(lines) if !lines.is_empty() => {
                    format!("\nClaude output:\n{}", lines.join("\n"))
                }
                _ => String::new(),
            };
            anyhow::bail!("claude exited with status: {status}{error_context}");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_github_envs_preserves_parent_when_empty() {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg("printf \"%s|%s|%s\" \"$GH_TOKEN\" \"$GITHUB_TOKEN\" \"$COPILOT_GITHUB_TOKEN\"");

        // simulate parent env present on the Command
        cmd.env("GH_TOKEN", "parent_gh").env("GITHUB_TOKEN", "parent_gh").env("COPILOT_GITHUB_TOKEN", "parent_copilot");

        set_github_envs(&mut cmd, "", "");
        let output = cmd.output().await.expect("failed to run");
        let out = String::from_utf8_lossy(&output.stdout);
        assert_eq!(out, "parent_gh|parent_gh|parent_copilot");
    }

    #[tokio::test]
    async fn test_set_github_envs_overrides_when_non_empty() {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg("printf \"%s|%s|%s\" \"$GH_TOKEN\" \"$GITHUB_TOKEN\" \"$COPILOT_GITHUB_TOKEN\"");

        set_github_envs(&mut cmd, "agent_gh", "agent_copilot");
        let output = cmd.output().await.expect("failed to run");
        let out = String::from_utf8_lossy(&output.stdout);
        assert_eq!(out, "agent_gh|agent_gh|agent_copilot");
    }
}

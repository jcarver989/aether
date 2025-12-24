use async_trait::async_trait;
use crucible::hooks::{Hook, HookInput, HookResult};
use std::{fs, process::Stdio};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Hook that reads a prompt file and executes it using Claude Code CLI
pub struct ClaudeCode {
    prompt_file: String,
}

impl ClaudeCode {
    pub fn new(plan_file: impl Into<String>) -> Self {
        Self {
            prompt_file: plan_file.into(),
        }
    }
}

#[async_trait]
impl Hook for ClaudeCode {
    async fn run(&self, input: HookInput) -> HookResult {
        let plan_path = input.working_directory.join(&self.prompt_file);
        let plan_contents = fs::read_to_string(&plan_path)
            .map_err(|e| format!("Failed to read {}: {}", self.prompt_file, e))?;

        tracing::info!("Starting Claude Code agent");

        let mut child = tokio::process::Command::new("claude")
            .arg("-p")
            .arg("--dangerously-skip-permissions")
            .arg("--verbose")
            .arg("--output-format")
            .arg("stream-json")
            .arg(format!("An engineer has produced this implementation plan: <plan>{}</plan>. Implement the feature per the plan.", plan_contents))
            .current_dir(&input.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn claude CLI: {}", e))?;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::info!("claude: {}", line);
                }
            });
        }

        let status = child
            .wait()
            .await
            .map_err(|e| format!("Failed to wait for claude CLI: {}", e))?;

        if !status.success() {
            return Err(format!("Claude CLI failed with status {}", status).into());
        }

        tracing::info!("Claude Code successfully executed the plan");

        Ok(())
    }
}

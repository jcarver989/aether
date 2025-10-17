use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BashInput {
    /// The command to execute
    pub command: String,
    /// Optional timeout in milliseconds (max 600000)
    pub timeout: Option<u64>,
    /// Clear, concise description of what this command does in 5-10 words
    pub description: Option<String>,
    /// Set to true to run this command in the background
    pub run_in_background: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BashOutput {
    /// Combined stdout and stderr output
    pub output: String,
    /// Exit code of the command
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
    /// Whether the command was killed due to timeout
    pub killed: Option<bool>,
    /// Shell ID for background processes
    #[serde(rename = "shellId")]
    pub shell_id: Option<String>,
}

// Handle for a background process
#[derive(Debug)]
pub struct BackgroundProcessHandle {
    pub shell_id: String,
    pub output_rx: mpsc::UnboundedReceiver<String>,
    pub task_handle: JoinHandle<(i32, bool)>,
}

#[derive(Debug)]
pub enum BashResult {
    Completed(BashOutput),
    Background(BackgroundProcessHandle),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadBackgroundBashInput {
    /// The ID of the background shell to retrieve output from
    pub bash_id: String,
    /// Optional regex to filter output lines
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadBackgroundBashOutput {
    /// New output since last check
    pub output: String,
    /// Current shell status
    pub status: String, // "running" | "completed" | "failed"
    /// Exit code (when completed)
    #[serde(rename = "exitCode", skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

pub async fn read_background_bash(
    handle: BackgroundProcessHandle,
    filter: Option<String>,
) -> Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), String> {
    let BackgroundProcessHandle {
        shell_id,
        mut output_rx,
        task_handle,
    } = handle;

    // Collect all available output
    let mut output = String::new();
    let filter_regex = if let Some(pattern) = filter {
        Some(regex::Regex::new(&pattern).map_err(|e| format!("Invalid regex: {}", e))?)
    } else {
        None
    };

    while let Ok(line) = output_rx.try_recv() {
        if let Some(ref regex) = filter_regex {
            if regex.is_match(&line) {
                output.push_str(&line);
            }
        } else {
            output.push_str(&line);
        }
    }

    // Check if task is finished
    if task_handle.is_finished() {
        let (exit_code, killed) = task_handle
            .await
            .map_err(|e| format!("Failed to join task: {}", e))?;

        let status = if killed {
            "failed".to_string()
        } else {
            "completed".to_string()
        };

        Ok((
            ReadBackgroundBashOutput {
                output,
                status,
                exit_code: Some(exit_code),
            },
            None,
        ))
    } else {
        Ok((
            ReadBackgroundBashOutput {
                output,
                status: "running".to_string(),
                exit_code: None,
            },
            Some(BackgroundProcessHandle {
                shell_id,
                output_rx,
                task_handle,
            }),
        ))
    }
}

async fn run_command_with_timeout(
    command: String,
    timeout: Option<Duration>,
    output_tx: mpsc::UnboundedSender<String>,
) -> (i32, bool) {
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg(&command);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let tx_clone = output_tx.clone();
    let run_command = async move {
        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;

        // Stream stdout
        if let Some(stdout) = child.stdout.take() {
            let tx = tx_clone.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(line + "\n");
                }
            });
        }

        // Stream stderr
        if let Some(stderr) = child.stderr.take() {
            let tx = tx_clone.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(line + "\n");
                }
            });
        }

        let status = child.wait().await.map_err(|e| format!("Wait failed: {}", e))?;
        Ok::<_, String>((status.code().unwrap_or(-1), false))
    };

    if let Some(timeout_duration) = timeout {
        match tokio::time::timeout(timeout_duration, run_command).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => (-1, false),
            Err(_) => {
                let _ = output_tx.send("Command timed out\n".to_string());
                (-1, true)
            }
        }
    } else {
        run_command.await.unwrap_or((-1, false))
    }
}

pub async fn execute_command(args: BashInput) -> Result<BashResult, String> {
    if args.command.trim() == "rm" {
        return Err("No you can't fucking delete files".to_string());
    }

    // Validate timeout is within bounds
    if let Some(timeout_ms) = args.timeout {
        if timeout_ms > 600000 {
            return Err("Timeout cannot exceed 600000ms (10 minutes)".to_string());
        }
    }

    let run_in_background = args.run_in_background.unwrap_or(false);
    let timeout_duration = args.timeout.map(Duration::from_millis);

    if run_in_background {
        // Spawn background process
        let shell_id = Uuid::new_v4().to_string();
        let command = args.command.clone();

        let (output_tx, output_rx) = mpsc::unbounded_channel();

        let task_handle = tokio::spawn(async move {
            run_command_with_timeout(command, timeout_duration, output_tx).await
        });

        Ok(BashResult::Background(BackgroundProcessHandle {
            shell_id,
            output_rx,
            task_handle,
        }))
    } else {
        // Run synchronously with default timeout of 120000ms (2 minutes)
        let timeout = timeout_duration.or(Some(Duration::from_millis(120000)));
        let command = args.command.clone();

        // Collect output in-memory for synchronous case
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&command);

        let result = if let Some(timeout_duration) = timeout {
            tokio::time::timeout(timeout_duration, cmd.output()).await
        } else {
            Ok(cmd.output().await)
        };

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                // Combine stdout and stderr
                let combined_output = if stderr.is_empty() {
                    stdout
                } else if stdout.is_empty() {
                    stderr
                } else {
                    format!("{}{}", stdout, stderr)
                };

                Ok(BashResult::Completed(BashOutput {
                    output: combined_output,
                    exit_code,
                    killed: Some(false),
                    shell_id: None,
                }))
            }
            Ok(Err(e)) => Err(format!("Failed to execute command '{}': {}", args.command, e)),
            Err(_) => {
                // Timeout occurred
                let timeout_ms = timeout.map(|d| d.as_millis()).unwrap_or(120000);
                Ok(BashResult::Completed(BashOutput {
                    output: format!("Command timed out after {}ms", timeout_ms),
                    exit_code: -1,
                    killed: Some(true),
                    shell_id: None,
                }))
            }
        }
    }
}

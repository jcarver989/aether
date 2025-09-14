use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BashArgs {
    /// The bash command to execute (e.g., "ls -la", "git status", "npm install")
    pub command: String,
    /// Working directory to execute the command in (defaults to current directory)
    pub working_dir: Option<String>,
}

pub async fn execute_command(args: BashArgs) -> Result<serde_json::Value, String> {
    let mut cmd = Command::new("bash");

    if args.command.trim() == "rm" {
        return Err("No you can't fucking delete files".to_string());
    }

    cmd.arg("-c").arg(&args.command);

    if let Some(working_dir) = &args.working_dir {
        let wd_path = Path::new(working_dir);
        if !wd_path.exists() {
            return Err(format!("Working directory does not exist: {}", working_dir));
        }
        if !wd_path.is_dir() {
            return Err(format!(
                "Working directory path is not a directory: {}",
                working_dir
            ));
        }
        cmd.current_dir(wd_path);
    }

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            Ok(serde_json::json!({
                "status": "success",
                "command": args.command,
                "working_dir": args.working_dir.unwrap_or_else(|| ".".to_string()),
                "exit_code": exit_code,
                "stdout": stdout,
                "stderr": stderr,
                "success": output.status.success()
            }))
        }
        Err(e) => Err(format!(
            "Failed to execute command '{}': {}",
            args.command, e
        )),
    }
}

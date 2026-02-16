use agent_client_protocol as acp;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub struct TerminalManager {
    next_id: u64,
    terminals: HashMap<String, Mutex<ManagedTerminal>>,
}

struct ManagedTerminal {
    child: Child,
    output_buf: String,
    output_byte_limit: Option<u64>,
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            terminals: HashMap::new(),
        }
    }

    pub async fn create_terminal(
        &mut self,
        req: &acp::CreateTerminalRequest,
    ) -> acp::Result<acp::CreateTerminalResponse> {
        let mut cmd = Command::new(&req.command);
        cmd.args(&req.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(cwd) = &req.cwd {
            cmd.current_dir(cwd);
        }

        for env_var in &req.env {
            cmd.env(&env_var.name, &env_var.value);
        }

        let child = cmd.spawn().map_err(|e| {
            acp::Error::internal_error().data(serde_json::json!(format!(
                "failed to spawn terminal command '{}': {e}",
                req.command
            )))
        })?;

        self.next_id += 1;
        let terminal_id = format!("term-{}", self.next_id);

        self.terminals.insert(
            terminal_id.clone(),
            Mutex::new(ManagedTerminal {
                child,
                output_buf: String::new(),
                output_byte_limit: req.output_byte_limit,
            }),
        );

        Ok(acp::CreateTerminalResponse::new(terminal_id))
    }

    pub async fn terminal_output(
        &self,
        req: &acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        let id = req.terminal_id.0.as_ref();
        let terminal_lock = self
            .terminals
            .get(id)
            .ok_or_else(|| acp::Error::invalid_params().data(serde_json::json!("unknown terminal")))?;

        let mut terminal = terminal_lock.lock().await;
        drain_output(&mut terminal).await;

        let truncated = truncate_output(&mut terminal);

        let exit_status = terminal
            .child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| {
                acp::TerminalExitStatus::new().exit_code(status.code().map(|c| c as u32))
            });

        Ok(acp::TerminalOutputResponse::new(&terminal.output_buf, truncated)
            .exit_status(exit_status))
    }

    pub async fn wait_for_terminal_exit(
        &self,
        req: &acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        let id = req.terminal_id.0.as_ref();
        let terminal_lock = self
            .terminals
            .get(id)
            .ok_or_else(|| acp::Error::invalid_params().data(serde_json::json!("unknown terminal")))?;

        let mut terminal = terminal_lock.lock().await;
        let status = terminal.child.wait().await.map_err(|e| {
            acp::Error::internal_error().data(serde_json::json!(format!("wait failed: {e}")))
        })?;

        drain_output(&mut terminal).await;

        let exit_status = acp::TerminalExitStatus::new().exit_code(status.code().map(|c| c as u32));
        Ok(acp::WaitForTerminalExitResponse::new(exit_status))
    }

    pub async fn release_terminal(
        &mut self,
        req: &acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        let id = req.terminal_id.0.as_ref();
        if let Some(terminal_lock) = self.terminals.remove(id) {
            let mut terminal = terminal_lock.into_inner();
            let _ = terminal.child.kill().await;
        }
        Ok(acp::ReleaseTerminalResponse::new())
    }

    pub async fn kill_terminal_command(
        &self,
        req: &acp::KillTerminalCommandRequest,
    ) -> acp::Result<acp::KillTerminalCommandResponse> {
        let id = req.terminal_id.0.as_ref();
        let terminal_lock = self
            .terminals
            .get(id)
            .ok_or_else(|| acp::Error::invalid_params().data(serde_json::json!("unknown terminal")))?;

        let mut terminal = terminal_lock.lock().await;
        let _ = terminal.child.kill().await;
        Ok(acp::KillTerminalCommandResponse::new())
    }
}

async fn drain_output(terminal: &mut ManagedTerminal) {
    let mut buf = [0u8; 4096];

    if let Some(stdout) = terminal.child.stdout.as_mut() {
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_millis(10),
                stdout.read(&mut buf),
            )
            .await
            {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        terminal.output_buf.push_str(s);
                    }
                }
                Ok(Err(_)) => break,
            }
        }
    }

    if let Some(stderr) = terminal.child.stderr.as_mut() {
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_millis(10),
                stderr.read(&mut buf),
            )
            .await
            {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        terminal.output_buf.push_str(s);
                    }
                }
                Ok(Err(_)) => break,
            }
        }
    }
}

fn truncate_output(terminal: &mut ManagedTerminal) -> bool {
    if let Some(limit) = terminal.output_byte_limit {
        let limit = limit as usize;
        if terminal.output_buf.len() > limit {
            // Find a valid char boundary near the truncation point
            let start = terminal.output_buf.len() - limit;
            let start = terminal
                .output_buf
                .ceil_char_boundary(start);
            terminal.output_buf = terminal.output_buf[start..].to_string();
            return true;
        }
    }
    false
}

//! ACP Client implementation for the desktop app.
//!
//! This module implements the `Client` trait from agent-client-protocol,
//! allowing the desktop app to communicate with any ACP-compatible agent.

use agent_client_protocol::{
    Client, ClientCapabilities, CreateTerminalRequest, CreateTerminalResponse, Error,
    FileSystemCapability, ReadTextFileRequest, ReadTextFileResponse, ReleaseTerminalRequest,
    ReleaseTerminalResponse, RequestPermissionRequest, RequestPermissionResponse, Result,
    SessionNotification, TerminalExitStatus, TerminalId, TerminalOutputRequest,
    TerminalOutputResponse, WaitForTerminalExitRequest, WaitForTerminalExitResponse,
    WriteTextFileRequest, WriteTextFileResponse,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Stdio;
use std::rc::Rc;
use tokio::fs::{create_dir_all, write};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

/// Raw events from AcpClient before session_id is attached.
///
/// These are transformed into `AgentEvent` by the agent task loop
/// which has access to the session_id.
#[derive(Debug)]
pub enum RawAgentEvent {
    SessionNotification(SessionNotification),
    PermissionRequest {
        request: RequestPermissionRequest,
        response_tx: oneshot::Sender<RequestPermissionResponse>,
    },
}

/// Tracks a running terminal process
struct TerminalProcess {
    child: tokio::process::Child,
    output: String,
    exit_status: Option<TerminalExitStatus>,
}

/// Shared terminal state (Rc because ?Send context)
#[derive(Clone, Default)]
struct TerminalState {
    terminals: Rc<RefCell<HashMap<String, TerminalProcess>>>,
}

/// ACP Client implementation for the desktop app.
///
/// This handles requests from the agent:
/// - File system operations (read/write)
/// - Terminal operations (create/output/release)
/// - Permission requests (forwarded to UI)
/// - Session notifications (forwarded to UI)
pub struct AcpClient {
    /// Channel to send raw events to the agent task loop
    event_tx: mpsc::UnboundedSender<RawAgentEvent>,
    /// Terminal state for managing spawned processes
    terminal_state: TerminalState,
}

impl AcpClient {
    pub fn new(event_tx: mpsc::UnboundedSender<RawAgentEvent>) -> Self {
        Self {
            event_tx,
            terminal_state: TerminalState::default(),
        }
    }

    pub fn capabilities() -> ClientCapabilities {
        ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Client for AcpClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> Result<RequestPermissionResponse> {
        debug!("Permission request: {:?}", args.tool_call);
        let (response_tx, response_rx) = oneshot::channel();
        let err = || Error::internal_error().with_data("Permission response channel closed");
        self.event_tx
            .send(RawAgentEvent::PermissionRequest {
                request: args,
                response_tx,
            })
            .map_err(|_| err())?;

        response_rx.await.map_err(|_| err())
    }

    async fn session_notification(&self, notification: SessionNotification) -> Result<()> {
        debug!("Session notification: {:?}", notification.update);
        self.event_tx
            .send(RawAgentEvent::SessionNotification(notification))
            .map_err(|_| Error::internal_error().with_data("Notification channel closed"))?;

        Ok(())
    }

    async fn read_text_file(&self, args: ReadTextFileRequest) -> Result<ReadTextFileResponse> {
        debug!("Read text file: {:?}", args.path);

        let content = tokio::fs::read_to_string(&args.path)
            .await
            .map_err(|e| Error::internal_error().with_data(e.to_string()))?;

        let content = if args.line.is_some() || args.limit.is_some() {
            let lines: Vec<&str> = content.lines().collect();
            let start = args.line.unwrap_or(1).saturating_sub(1) as usize;
            let limit = args.limit.map(|l| l as usize).unwrap_or(lines.len());

            lines
                .into_iter()
                .skip(start)
                .take(limit)
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            content
        };

        Ok(ReadTextFileResponse {
            content,
            meta: None,
        })
    }

    async fn write_text_file(&self, args: WriteTextFileRequest) -> Result<WriteTextFileResponse> {
        debug!("Write text file: {:?}", args.path);

        if let Some(parent) = args.path.parent() {
            create_dir_all(parent).await.map_err(|e| {
                agent_client_protocol::Error::internal_error().with_data(e.to_string())
            })?;
        }

        write(&args.path, &args.content)
            .await
            .map_err(|e| agent_client_protocol::Error::internal_error().with_data(e.to_string()))?;

        Ok(WriteTextFileResponse { meta: None })
    }

    async fn create_terminal(&self, args: CreateTerminalRequest) -> Result<CreateTerminalResponse> {
        debug!("Create terminal: {}", args.command);
        let terminal_id = TerminalId::from(uuid::Uuid::new_v4().to_string());
        let child = {
            let mut cmd = Command::new("bash");
            cmd.arg("-c").arg(&args.command);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            if let Some(cwd) = &args.cwd {
                cmd.current_dir(cwd);
            }

            for env_var in &args.env {
                cmd.env(&env_var.name, &env_var.value);
            }

            cmd.spawn()
                .map_err(|e| Error::internal_error().with_data(format!("Failed to spawn: {e}")))?
        };

        let terminal_process = TerminalProcess {
            child,
            output: String::new(),
            exit_status: None,
        };

        self.terminal_state
            .terminals
            .borrow_mut()
            .insert(terminal_id.to_string(), terminal_process);

        Ok(CreateTerminalResponse {
            terminal_id,
            meta: None,
        })
    }

    async fn terminal_output(&self, args: TerminalOutputRequest) -> Result<TerminalOutputResponse> {
        debug!("Terminal output: {:?}", args.terminal_id);

        let terminal_id = args.terminal_id.to_string();
        let mut terminals = self.terminal_state.terminals.borrow_mut();
        let terminal = terminals.get_mut(&terminal_id).ok_or_else(|| {
            Error::internal_error().with_data(format!("Terminal not found: {terminal_id}"))
        })?;

        if terminal.exit_status.is_none() {
            if let Ok(Some(status)) = terminal.child.try_wait() {
                terminal.exit_status = Some(TerminalExitStatus {
                    exit_code: status.code().map(|c| c as u32),
                    signal: None,
                    meta: None,
                });
            }
        }

        Ok(TerminalOutputResponse {
            output: terminal.output.clone(),
            truncated: false,
            exit_status: terminal.exit_status.clone(),
            meta: None,
        })
    }

    async fn wait_for_terminal_exit(
        &self,
        args: WaitForTerminalExitRequest,
    ) -> Result<WaitForTerminalExitResponse> {
        debug!("Wait for terminal exit: {:?}", args.terminal_id);
        let terminal_id = args.terminal_id.to_string();
        let mut terminal = {
            let mut terminals = self.terminal_state.terminals.borrow_mut();
            terminals.remove(&terminal_id).ok_or_else(|| {
                Error::internal_error().with_data(format!("Terminal not found: {terminal_id}"))
            })?
        };

        let status = terminal
            .child
            .wait()
            .await
            .map_err(|e| Error::internal_error().with_data(format!("Wait failed: {e}")))?;

        if let Some(mut stdout) = terminal.child.stdout.take() {
            let mut buf = String::new();
            if stdout.read_to_string(&mut buf).await.is_ok() {
                terminal.output.push_str(&buf);
            }
        }

        if let Some(mut stderr) = terminal.child.stderr.take() {
            let mut buf = String::new();
            if stderr.read_to_string(&mut buf).await.is_ok() {
                terminal.output.push_str(&buf);
            }
        }

        let exit_status = TerminalExitStatus {
            exit_code: status.code().map(|c| c as u32),
            signal: None,
            meta: None,
        };

        terminal.exit_status = Some(exit_status.clone());
        self.terminal_state
            .terminals
            .borrow_mut()
            .insert(terminal_id, terminal);

        Ok(WaitForTerminalExitResponse {
            exit_status,
            meta: None,
        })
    }

    async fn release_terminal(
        &self,
        args: ReleaseTerminalRequest,
    ) -> Result<ReleaseTerminalResponse> {
        debug!("Release terminal: {:?}", args.terminal_id);

        let terminal_id = args.terminal_id.to_string();
        let mut terminals = self.terminal_state.terminals.borrow_mut();

        if let Some(mut terminal) = terminals.remove(&terminal_id) {
            // Kill if still running
            let _ = terminal.child.kill().await;
        }

        Ok(ReleaseTerminalResponse { meta: None })
    }
}

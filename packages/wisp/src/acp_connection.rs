use agent_client_protocol as acp;
use acp::Agent;
use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::info;

use crate::error::WispError;
use crate::terminal_manager::TerminalManager;

/// Events forwarded from the ACP connection to the main event loop.
pub enum AcpEvent {
    SessionUpdate(Box<acp::SessionUpdate>),
    PromptDone(acp::StopReason),
    PromptError(acp::Error),
    ConnectionClosed,
}

/// Commands sent from the main thread to the ACP LocalSet thread.
enum PromptCommand {
    Prompt {
        session_id: acp::SessionId,
        text: String,
    },
    #[allow(dead_code)]
    Cancel {
        session_id: acp::SessionId,
    },
}

/// Send-safe handle for issuing prompt commands to the !Send ACP connection.
#[derive(Clone)]
pub struct AcpPromptHandle {
    cmd_tx: mpsc::UnboundedSender<PromptCommand>,
}

impl AcpPromptHandle {
    /// Create a handle that discards all commands. Useful for testing.
    #[allow(dead_code)]
    pub fn disconnected() -> Self {
        let (cmd_tx, _rx) = mpsc::unbounded_channel();
        Self { cmd_tx }
    }

    pub fn prompt(&self, session_id: &acp::SessionId, text: &str) {
        let _ = self.cmd_tx.send(PromptCommand::Prompt {
            session_id: session_id.clone(),
            text: text.to_string(),
        });
    }

    #[allow(dead_code)]
    pub fn cancel(&self, session_id: &acp::SessionId) {
        let _ = self.cmd_tx.send(PromptCommand::Cancel {
            session_id: session_id.clone(),
        });
    }
}

pub struct AcpSession {
    pub session_id: acp::SessionId,
    pub event_rx: mpsc::UnboundedReceiver<AcpEvent>,
    pub prompt_handle: AcpPromptHandle,
}

/// Spawn the agent subprocess and establish an ACP session.
///
/// The handshake (initialize + new_session) runs on a dedicated !Send thread.
/// Returns the session info, an event receiver, and a prompt handle.
pub async fn spawn_acp_session(agent_command: &str) -> Result<AcpSession, WispError> {
    let parts: Vec<&str> = agent_command.split_whitespace().collect();
    let (program, args) = parts
        .split_first()
        .ok_or_else(|| WispError::AgentCrashed("empty agent command".to_string()))?;

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(WispError::AgentSpawnFailed)?;

    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| WispError::AgentCrashed("no stdin on child".to_string()))?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| WispError::AgentCrashed("no stdout on child".to_string()))?;

    let (event_tx, event_rx) = mpsc::unbounded_channel::<AcpEvent>();
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<PromptCommand>();
    let (session_tx, session_rx) =
        tokio::sync::oneshot::channel::<Result<acp::SessionId, WispError>>();

    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime for ACP");

        let local_set = tokio::task::LocalSet::new();
        local_set.block_on(&rt, async move {
            run_acp_thread(child_stdin, child_stdout, event_tx, cmd_rx, session_tx, cwd).await;
        });
    });

    // Wait for the handshake result from the LocalSet thread
    let session_id = session_rx
        .await
        .map_err(|_| WispError::AgentCrashed("ACP thread died during handshake".to_string()))??;

    Ok(AcpSession {
        session_id,
        event_rx,
        prompt_handle: AcpPromptHandle { cmd_tx },
    })
}

/// The ACP Client implementation that handles callbacks from the agent.
struct WispClient {
    event_tx: mpsc::UnboundedSender<AcpEvent>,
    terminal_manager: RefCell<TerminalManager>,
}

// Safety: WispClient runs on a single-threaded LocalSet, so RefCell borrows
// across await points cannot actually interleave.
#[allow(clippy::await_holding_refcell_ref)]
#[async_trait::async_trait(?Send)]
impl acp::Client for WispClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        // Auto-approve: pick the first AllowOnce or AllowAlways option
        let option_id = args
            .options
            .iter()
            .find(|o| {
                matches!(
                    o.kind,
                    acp::PermissionOptionKind::AllowOnce
                        | acp::PermissionOptionKind::AllowAlways
                )
            })
            .map(|o| o.option_id.clone())
            .unwrap_or_else(|| args.options[0].option_id.clone());

        Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                option_id,
            )),
        ))
    }

    async fn session_notification(&self, args: acp::SessionNotification) -> acp::Result<()> {
        let _ = self
            .event_tx
            .send(AcpEvent::SessionUpdate(Box::new(args.update)));
        Ok(())
    }

    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        let content = std::fs::read_to_string(&args.path).map_err(|e| {
            acp::Error::internal_error().data(serde_json::json!(format!("read error: {e}")))
        })?;

        let content = match (args.line, args.limit) {
            (Some(line), Some(limit)) => {
                let start = (line as usize).saturating_sub(1);
                content
                    .lines()
                    .skip(start)
                    .take(limit as usize)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (Some(line), None) => {
                let start = (line as usize).saturating_sub(1);
                content.lines().skip(start).collect::<Vec<_>>().join("\n")
            }
            (None, Some(limit)) => content
                .lines()
                .take(limit as usize)
                .collect::<Vec<_>>()
                .join("\n"),
            (None, None) => content,
        };

        Ok(acp::ReadTextFileResponse::new(content))
    }

    async fn write_text_file(
        &self,
        args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        if let Some(parent) = args.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                acp::Error::internal_error().data(serde_json::json!(format!("mkdir error: {e}")))
            })?;
        }
        std::fs::write(&args.path, &args.content).map_err(|e| {
            acp::Error::internal_error().data(serde_json::json!(format!("write error: {e}")))
        })?;
        Ok(acp::WriteTextFileResponse::new())
    }

    async fn create_terminal(
        &self,
        args: acp::CreateTerminalRequest,
    ) -> acp::Result<acp::CreateTerminalResponse> {
        self.terminal_manager.borrow_mut().create_terminal(&args).await
    }

    async fn terminal_output(
        &self,
        args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        self.terminal_manager.borrow().terminal_output(&args).await
    }

    async fn wait_for_terminal_exit(
        &self,
        args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        self.terminal_manager
            .borrow()
            .wait_for_terminal_exit(&args)
            .await
    }

    async fn release_terminal(
        &self,
        args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        self.terminal_manager
            .borrow_mut()
            .release_terminal(&args)
            .await
    }

    async fn kill_terminal_command(
        &self,
        args: acp::KillTerminalCommandRequest,
    ) -> acp::Result<acp::KillTerminalCommandResponse> {
        self.terminal_manager
            .borrow()
            .kill_terminal_command(&args)
            .await
    }
}

async fn run_acp_thread(
    child_stdin: tokio::process::ChildStdin,
    child_stdout: tokio::process::ChildStdout,
    event_tx: mpsc::UnboundedSender<AcpEvent>,
    mut cmd_rx: mpsc::UnboundedReceiver<PromptCommand>,
    session_tx: tokio::sync::oneshot::Sender<Result<acp::SessionId, WispError>>,
    cwd: PathBuf,
) {
    let client = WispClient {
        event_tx: event_tx.clone(),
        terminal_manager: RefCell::new(TerminalManager::new()),
    };

    let outgoing = child_stdin.compat_write();
    let incoming = child_stdout.compat();

    let (conn, handle_io) = acp::ClientSideConnection::new(client, outgoing, incoming, |fut| {
        tokio::task::spawn_local(fut);
    });

    // Spawn IO handler first so the connection can process messages
    tokio::task::spawn_local(async move {
        let _ = handle_io.await;
    });

    // Initialize
    let init_resp = match conn
        .initialize(
            acp::InitializeRequest::new(acp::ProtocolVersion::LATEST)
                .client_capabilities(
                    acp::ClientCapabilities::new()
                        .fs(
                            acp::FileSystemCapability::new()
                                .read_text_file(true)
                                .write_text_file(true),
                        )
                        .terminal(true),
                )
                .client_info(acp::Implementation::new("wisp", env!("CARGO_PKG_VERSION"))),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = session_tx.send(Err(WispError::AcpError(e)));
            return;
        }
    };

    info!(
        "ACP initialized: protocol={:?}, agent_info={:?}",
        init_resp.protocol_version, init_resp.agent_info
    );

    // Authenticate if needed
    if !init_resp.auth_methods.is_empty() {
        let method_id = init_resp.auth_methods[0].id.clone();
        if let Err(e) = conn
            .authenticate(acp::AuthenticateRequest::new(method_id))
            .await
        {
            let _ = session_tx.send(Err(WispError::AcpError(e)));
            return;
        }
    }

    // Create session
    let session_resp = match conn.new_session(acp::NewSessionRequest::new(cwd)).await {
        Ok(r) => r,
        Err(e) => {
            let _ = session_tx.send(Err(WispError::AcpError(e)));
            return;
        }
    };

    let session_id = session_resp.session_id;
    info!("ACP session created: {session_id}");

    // Send session_id back to the main thread
    let _ = session_tx.send(Ok(session_id));

    // Process prompt commands
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            PromptCommand::Prompt { session_id, text } => {
                let prompt = vec![acp::ContentBlock::Text(acp::TextContent::new(text))];
                match conn
                    .prompt(acp::PromptRequest::new(session_id, prompt))
                    .await
                {
                    Ok(resp) => {
                        let _ = event_tx.send(AcpEvent::PromptDone(resp.stop_reason));
                    }
                    Err(e) => {
                        let _ = event_tx.send(AcpEvent::PromptError(e));
                    }
                }
            }
            PromptCommand::Cancel { session_id } => {
                let _ = conn.cancel(acp::CancelNotification::new(session_id)).await;
            }
        }
    }

    let _ = event_tx.send(AcpEvent::ConnectionClosed);
}

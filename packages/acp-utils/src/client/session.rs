use agent_client_protocol::{
    self as acp, Agent, Implementation, InitializeRequest, ProtocolVersion,
};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::info;

use super::error::AcpClientError;
use super::event::AcpEvent;
use super::prompt_handle::{AcpPromptHandle, PromptCommand};

/// Configuration for spawning an ACP session.
pub struct SpawnConfig {
    /// The agent command to run (e.g., "aether-acp --model anthropic:claude-3.5-sonnet").
    pub agent_command: String,

    /// Client name reported during ACP initialization.
    pub client_name: String,

    /// Client version reported during ACP initialization.
    pub client_version: String,

    /// Working directory for the ACP session.
    pub cwd: PathBuf,
}

/// Completed ACP session with all handles needed by the caller.
pub struct AcpSession {
    pub session_id: acp::SessionId,
    pub agent_name: String,
    pub config_options: Vec<acp::SessionConfigOption>,
    pub event_rx: mpsc::UnboundedReceiver<AcpEvent>,
    pub prompt_handle: AcpPromptHandle,
}

/// A built-in ACP client that auto-approves permissions and forwards session
/// notifications as [`AcpEvent`]s.
pub struct AutoApproveClient {
    event_tx: mpsc::UnboundedSender<AcpEvent>,
}

impl AutoApproveClient {
    pub fn new(event_tx: mpsc::UnboundedSender<AcpEvent>) -> Self {
        Self { event_tx }
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for AutoApproveClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let option_id = args
            .options
            .iter()
            .find(|o| {
                matches!(
                    o.kind,
                    acp::PermissionOptionKind::AllowOnce | acp::PermissionOptionKind::AllowAlways
                )
            })
            .map(|o| o.option_id.clone())
            .unwrap_or_else(|| args.options[0].option_id.clone());

        Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id)),
        ))
    }

    async fn session_notification(&self, args: acp::SessionNotification) -> acp::Result<()> {
        let _ = self
            .event_tx
            .send(AcpEvent::SessionUpdate(Box::new(args.update)));
        Ok(())
    }
}

/// Spawn an agent subprocess and establish an ACP session.
///
/// The handshake (initialize + new_session) runs on a dedicated !Send thread.
/// `client_factory` creates the ACP [`Client`](acp::Client) implementation,
/// receiving the event sender so it can forward protocol events.
///
/// For the common auto-approve case, use `AutoApproveClient::new`:
/// ```ignore
/// spawn_acp_session(config, AutoApproveClient::new).await
/// ```
pub async fn spawn_acp_session<F, C>(
    config: SpawnConfig,
    client_factory: F,
) -> Result<AcpSession, AcpClientError>
where
    F: FnOnce(mpsc::UnboundedSender<AcpEvent>) -> C + Send + 'static,
    C: acp::Client + 'static,
{
    let parts: Vec<&str> = config.agent_command.split_whitespace().collect();
    let (program, args) = parts
        .split_first()
        .ok_or_else(|| AcpClientError::AgentCrashed("empty agent command".to_string()))?;

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(AcpClientError::SpawnFailed)?;

    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| AcpClientError::AgentCrashed("no stdin on child".to_string()))?;

    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| AcpClientError::AgentCrashed("no stdout on child".to_string()))?;

    let (event_tx, event_rx) = mpsc::unbounded_channel::<AcpEvent>();
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<PromptCommand>();

    let (session_tx, session_rx) = tokio::sync::oneshot::channel::<HandshakeResult>();

    let thread_ctx = AcpThreadContext {
        child_stdin,
        child_stdout,
        event_tx,
        cmd_rx,
        session_tx,
        client_factory,
        client_name: config.client_name,
        client_version: config.client_version,
        cwd: config.cwd,
    };

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime for ACP");

        let local_set = tokio::task::LocalSet::new();
        local_set.block_on(&rt, async move {
            run_acp_thread(thread_ctx).await;
        });
    });

    let (session_id, agent_name, config_options) = session_rx
        .await
        .map_err(|_| {
            AcpClientError::AgentCrashed("ACP thread died during handshake".to_string())
        })??;

    Ok(AcpSession {
        session_id,
        agent_name,
        config_options,
        event_rx,
        prompt_handle: AcpPromptHandle { cmd_tx },
    })
}

type HandshakeResult =
    Result<(acp::SessionId, String, Vec<acp::SessionConfigOption>), AcpClientError>;

struct AcpThreadContext<F> {
    child_stdin: tokio::process::ChildStdin,
    child_stdout: tokio::process::ChildStdout,
    event_tx: mpsc::UnboundedSender<AcpEvent>,
    cmd_rx: mpsc::UnboundedReceiver<PromptCommand>,
    session_tx: tokio::sync::oneshot::Sender<HandshakeResult>,
    client_factory: F,
    client_name: String,
    client_version: String,
    cwd: PathBuf,
}

async fn run_acp_thread<F, C>(ctx: AcpThreadContext<F>)
where
    F: FnOnce(mpsc::UnboundedSender<AcpEvent>) -> C,
    C: acp::Client + 'static,
{
    let AcpThreadContext {
        child_stdin,
        child_stdout,
        event_tx,
        mut cmd_rx,
        session_tx,
        client_factory,
        client_name,
        client_version,
        cwd,
    } = ctx;

    let client = client_factory(event_tx.clone());

    let outgoing = child_stdin.compat_write();
    let incoming = child_stdout.compat();

    let (conn, handle_io) = acp::ClientSideConnection::new(client, outgoing, incoming, |fut| {
        tokio::task::spawn_local(fut);
    });

    tokio::task::spawn_local(async move {
        let _ = handle_io.await;
    });

    let init_resp = match conn
        .initialize(
            InitializeRequest::new(ProtocolVersion::LATEST)
                .client_info(Implementation::new(client_name, client_version)),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = session_tx.send(Err(AcpClientError::Protocol(e)));
            return;
        }
    };

    let agent_name = init_resp
        .agent_info
        .as_ref()
        .map(|info| info.title.as_deref().unwrap_or(&info.name).to_string())
        .unwrap_or_else(|| "agent".to_string());

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
            let _ = session_tx.send(Err(AcpClientError::Protocol(e)));
            return;
        }
    }

    // Create session
    let session_resp = match conn.new_session(acp::NewSessionRequest::new(cwd)).await {
        Ok(r) => r,
        Err(e) => {
            let _ = session_tx.send(Err(AcpClientError::Protocol(e)));
            return;
        }
    };

    let session_id = session_resp.session_id;
    let config_options = session_resp.config_options.unwrap_or_default();
    info!("ACP session created: {session_id}");

    let _ = session_tx.send(Ok((session_id, agent_name, config_options)));

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

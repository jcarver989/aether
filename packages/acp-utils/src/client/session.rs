use super::error::AcpClientError;
use super::event::AcpEvent;
use super::prompt_handle::{AcpPromptHandle, PromptCommand};
use crate::notifications::{ELICITATION_METHOD, ElicitationParams, McpRequest};
use agent_client_protocol::{
    self as acp, Agent, Client, ConfigOptionUpdate, ExtNotification, ExtRequest, ExtResponse,
    InitializeRequest, PermissionOptionKind, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionConfigOption, SessionId,
    SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
};
use serde_json::value::RawValue;
use std::process::Stdio;
use std::sync::Arc;
use std::thread::spawn;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tokio::task::{LocalSet, spawn_local};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::info;

/// ACP session with all handles needed by the caller.
pub struct AcpSession {
    pub session_id: SessionId,
    pub agent_name: String,
    pub config_options: Vec<SessionConfigOption>,
    pub auth_methods: Vec<acp::AuthMethod>,
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
impl Client for AutoApproveClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> acp::Result<RequestPermissionResponse> {
        let option_id = args
            .options
            .iter()
            .find(|o| {
                matches!(
                    o.kind,
                    PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
                )
            })
            .map_or_else(
                || args.options[0].option_id.clone(),
                |o| o.option_id.clone(),
            );

        Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id)),
        ))
    }

    async fn session_notification(&self, args: SessionNotification) -> acp::Result<()> {
        let _ = self
            .event_tx
            .send(AcpEvent::SessionUpdate(Box::new(args.update)));

        Ok(())
    }

    async fn ext_notification(&self, args: ExtNotification) -> acp::Result<()> {
        let _ = self.event_tx.send(AcpEvent::ExtNotification(args));
        Ok(())
    }

    async fn ext_method(&self, args: ExtRequest) -> acp::Result<ExtResponse> {
        if args.method.as_ref() == ELICITATION_METHOD {
            return handle_elicitation_ext_method(&self.event_tx, args).await;
        }

        // Unknown ext_method — return null (default behavior)
        let null_raw: Arc<RawValue> = serde_json::from_str("null").expect("null is valid JSON");
        Ok(ExtResponse::new(null_raw))
    }
}

async fn handle_elicitation_ext_method(
    event_tx: &mpsc::UnboundedSender<AcpEvent>,
    args: ExtRequest,
) -> acp::Result<ExtResponse> {
    let params: ElicitationParams =
        serde_json::from_str(args.params.get()).map_err(|_| acp::Error::invalid_params())?;

    let (response_tx, response_rx) = oneshot::channel();
    event_tx
        .send(AcpEvent::ElicitationRequest {
            params,
            response_tx,
        })
        .map_err(|_| acp::Error::internal_error())?;

    let response = response_rx
        .await
        .map_err(|_| acp::Error::internal_error())?;

    let raw =
        serde_json::value::to_raw_value(&response).map_err(|_| acp::Error::internal_error())?;

    Ok(ExtResponse::new(Arc::from(raw)))
}

/// Spawn an agent subprocess and establish an ACP session.
///
/// The handshake (initialize + `new_session`) runs on a dedicated !Send thread.
/// `client_factory` creates the ACP [`Client`](acp::Client) implementation,
/// receiving the event sender so it can forward protocol events.
///
/// For the common auto-approve case, use `AutoApproveClient::new`:
/// ```ignore
/// spawn_acp_session("my-agent", init_req, session_req, AutoApproveClient::new).await
/// ```
pub async fn spawn_acp_session<F, C>(
    agent_command: &str,
    init_request: InitializeRequest,
    new_session_request: acp::NewSessionRequest,
    client_factory: F,
) -> Result<AcpSession, AcpClientError>
where
    F: FnOnce(mpsc::UnboundedSender<AcpEvent>) -> C + Send + 'static,
    C: acp::Client + 'static,
{
    let parts: Vec<&str> = agent_command.split_whitespace().collect();
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
    let (session_tx, session_rx) = oneshot::channel::<HandshakeResult>();
    let thread_ctx = AcpThreadContext {
        child_stdin,
        child_stdout,
        event_tx,
        cmd_rx,
        session_tx,
        client_factory,
        init_request,
        new_session_request,
    };

    spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime for ACP");

        LocalSet::new().block_on(&rt, async move {
            run_acp_thread(thread_ctx).await;
        });
    });

    let (session_id, agent_name, config_options, auth_methods) =
        session_rx.await.map_err(|_| {
            AcpClientError::AgentCrashed("ACP thread died during handshake".to_string())
        })??;

    Ok(AcpSession {
        session_id,
        agent_name,
        config_options,
        auth_methods,
        event_rx,
        prompt_handle: AcpPromptHandle { cmd_tx },
    })
}

type HandshakeResult = Result<
    (
        acp::SessionId,
        String,
        Vec<acp::SessionConfigOption>,
        Vec<acp::AuthMethod>,
    ),
    AcpClientError,
>;

struct AcpThreadContext<F> {
    child_stdin: tokio::process::ChildStdin,
    child_stdout: tokio::process::ChildStdout,
    event_tx: mpsc::UnboundedSender<AcpEvent>,
    cmd_rx: mpsc::UnboundedReceiver<PromptCommand>,
    session_tx: tokio::sync::oneshot::Sender<HandshakeResult>,
    client_factory: F,
    init_request: InitializeRequest,
    new_session_request: acp::NewSessionRequest,
}

#[allow(clippy::too_many_lines)]
async fn run_acp_thread<F, C>(ctx: AcpThreadContext<F>)
where
    F: FnOnce(mpsc::UnboundedSender<AcpEvent>) -> C,
    C: Client + 'static,
{
    let AcpThreadContext {
        child_stdin,
        child_stdout,
        event_tx,
        mut cmd_rx,
        session_tx,
        client_factory,
        init_request,
        new_session_request,
    } = ctx;

    let client = client_factory(event_tx.clone());
    let outgoing = child_stdin.compat_write();
    let incoming = child_stdout.compat();
    let (conn, handle_io) = acp::ClientSideConnection::new(client, outgoing, incoming, |fut| {
        spawn_local(fut);
    });

    spawn_local(async move {
        let _ = handle_io.await;
    });

    let init_resp = match conn.initialize(init_request).await {
        Ok(r) => r,
        Err(e) => {
            let _ = session_tx.send(Err(AcpClientError::Protocol(e)));
            return;
        }
    };

    let agent_name = init_resp.agent_info.as_ref().map_or_else(
        || "agent".to_string(),
        |info| info.title.as_deref().unwrap_or(&info.name).to_string(),
    );

    info!(
        "ACP initialized: protocol={:?}, agent_info={:?}",
        init_resp.protocol_version, init_resp.agent_info
    );

    let auth_methods = init_resp.auth_methods;

    let session_resp = match conn.new_session(new_session_request).await {
        Ok(r) => r,
        Err(e) => {
            let _ = session_tx.send(Err(AcpClientError::Protocol(e)));
            return;
        }
    };

    let session_id = session_resp.session_id;
    info!("ACP session created: {session_id}");

    let config_options = session_resp.config_options.unwrap_or_default();
    let _ = session_tx.send(Ok((session_id, agent_name, config_options, auth_methods)));

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            PromptCommand::Prompt {
                session_id,
                text,
                content,
            } => {
                let mut prompt = vec![acp::ContentBlock::Text(acp::TextContent::new(text))];
                if let Some(extra_content) = content {
                    prompt.extend(extra_content);
                }
                let prompt_fut = conn.prompt(acp::PromptRequest::new(session_id, prompt));
                tokio::pin!(prompt_fut);

                // Process cancel/config commands while the prompt is in-flight
                loop {
                    tokio::select! {
                        result = &mut prompt_fut => {
                            let event = match result {
                                Ok(resp) => AcpEvent::PromptDone(resp.stop_reason),
                                Err(e) => AcpEvent::PromptError(e),
                            };
                            let _ = event_tx.send(event);
                            break;
                        }
                        Some(cmd) = cmd_rx.recv() => {
                            handle_side_command(&conn, &event_tx, cmd).await;
                        }
                    }
                }
            }
            PromptCommand::ListSessions => {
                let req = acp::ListSessionsRequest::new();
                match conn.list_sessions(req).await {
                    Ok(resp) => {
                        let _ = event_tx.send(AcpEvent::SessionsListed {
                            sessions: resp.sessions,
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AcpEvent::PromptError(e));
                    }
                }
            }
            PromptCommand::LoadSession { session_id, cwd } => {
                let req = acp::LoadSessionRequest::new(session_id.clone(), cwd);
                match conn.load_session(req).await {
                    Ok(resp) => {
                        let config_options = resp.config_options.unwrap_or_default();
                        let _ = event_tx.send(AcpEvent::SessionLoaded {
                            session_id,
                            config_options,
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AcpEvent::PromptError(e));
                    }
                }
            }
            cmd => handle_side_command(&conn, &event_tx, cmd).await,
        }
    }

    let _ = event_tx.send(AcpEvent::ConnectionClosed);
}

async fn handle_side_command(
    conn: &acp::ClientSideConnection,
    event_tx: &mpsc::UnboundedSender<AcpEvent>,
    cmd: PromptCommand,
) {
    match cmd {
        PromptCommand::Cancel { session_id } => {
            let _ = conn.cancel(acp::CancelNotification::new(session_id)).await;
        }
        PromptCommand::SetConfigOption {
            session_id,
            config_id,
            value,
        } => {
            let req = SetSessionConfigOptionRequest::new(session_id, config_id, value);
            match conn.set_session_config_option(req).await {
                Ok(resp) => {
                    let update = ConfigOptionUpdate::new(resp.config_options);
                    let _ = event_tx.send(AcpEvent::SessionUpdate(Box::new(
                        SessionUpdate::ConfigOptionUpdate(update),
                    )));
                }
                Err(e) => {
                    tracing::warn!("set_session_config_option failed: {e:?}");
                }
            }
        }
        PromptCommand::Prompt { .. } => {
            tracing::warn!("ignoring duplicate Prompt while one is in-flight");
        }
        PromptCommand::ListSessions => {
            tracing::warn!("ignoring ListSessions while prompt is in-flight");
        }
        PromptCommand::LoadSession { .. } => {
            tracing::warn!("ignoring LoadSession while prompt is in-flight");
        }
        PromptCommand::AuthenticateMcpServer {
            session_id,
            server_name,
        } => {
            let msg = McpRequest::Authenticate {
                session_id: session_id.0.to_string(),
                server_name,
            };
            if let Err(e) = conn.ext_notification(msg.into()).await {
                tracing::warn!("authenticate_mcp_server notification failed: {e:?}");
            }
        }
        PromptCommand::Authenticate {
            session_id: _,
            method_id,
        } => {
            match conn
                .authenticate(acp::AuthenticateRequest::new(method_id.clone()))
                .await
            {
                Ok(_) => {
                    let _ = event_tx.send(AcpEvent::AuthenticateComplete { method_id });
                }
                Err(e) => {
                    tracing::warn!("authenticate failed: {e:?}");
                    let _ = event_tx.send(AcpEvent::AuthenticateFailed {
                        method_id,
                        error: format!("{e:?}"),
                    });
                }
            }
        }
    }
}

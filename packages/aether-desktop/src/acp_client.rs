//! ACP Client implementation for the desktop app.
//!
//! This module implements the `Client` trait from agent-client-protocol,
//! allowing the desktop app to communicate with any ACP-compatible agent.

use agent_client_protocol::{
    Client, ClientCapabilities, Error, FileSystemCapability, ReadTextFileRequest,
    ReadTextFileResponse, RequestPermissionRequest, RequestPermissionResponse, Result,
    SessionNotification, WriteTextFileRequest, WriteTextFileResponse,
};
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
}

impl AcpClient {
    pub fn new(event_tx: mpsc::UnboundedSender<RawAgentEvent>) -> Self {
        Self { event_tx }
    }

    pub fn capabilities() -> ClientCapabilities {
        ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: false,
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
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                agent_client_protocol::Error::internal_error().with_data(e.to_string())
            })?;
        }

        tokio::fs::write(&args.path, &args.content)
            .await
            .map_err(|e| agent_client_protocol::Error::internal_error().with_data(e.to_string()))?;

        Ok(WriteTextFileResponse { meta: None })
    }
}

use agent_client_protocol as acp;
use agent_client_protocol::Client;
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

/// Messages that can be sent to the ACP actor
#[derive(Debug)]
pub enum AcpRequest {
    ReadTextFile {
        request: acp::ReadTextFileRequest,
        response_tx: oneshot::Sender<Result<acp::ReadTextFileResponse, String>>,
    },
    WriteTextFile {
        request: acp::WriteTextFileRequest,
        response_tx: oneshot::Sender<Result<acp::WriteTextFileResponse, String>>,
    },
    CreateTerminal {
        request: acp::CreateTerminalRequest,
        response_tx: oneshot::Sender<Result<acp::CreateTerminalResponse, String>>,
    },
    WaitForTerminalExit {
        request: acp::WaitForTerminalExitRequest,
        response_tx: oneshot::Sender<Result<acp::WaitForTerminalExitResponse, String>>,
    },
    TerminalOutput {
        request: acp::TerminalOutputRequest,
        response_tx: oneshot::Sender<Result<acp::TerminalOutputResponse, String>>,
    },
    ReleaseTerminal {
        request: acp::ReleaseTerminalRequest,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    SessionNotification {
        notification: acp::SessionNotification,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    ExtNotification {
        notification: acp::ExtNotification,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}

/// Actor that owns the ACP connection and processes requests
/// ACP connections are not Send/Sync
pub struct AcpActor {
    conn: acp::AgentSideConnection,
    request_rx: mpsc::UnboundedReceiver<AcpRequest>,
}

impl AcpActor {
    pub fn new(
        conn: acp::AgentSideConnection,
        request_rx: mpsc::UnboundedReceiver<AcpRequest>,
    ) -> Self {
        Self { conn, request_rx }
    }

    /// Run the actor loop - this must be spawned on a LocalSet
    pub async fn run(mut self) {
        debug!("ACP actor starting");

        while let Some(request) = self.request_rx.recv().await {
            self.handle_request(request).await;
        }

        debug!("ACP actor stopping");
    }

    async fn handle_request(&self, request: AcpRequest) {
        match request {
            AcpRequest::ReadTextFile {
                request,
                response_tx,
            } => {
                debug!("ACP actor: read_text_file {:?}", request.path);
                let result = self
                    .conn
                    .read_text_file(request)
                    .await
                    .map_err(|e| format!("read_text_file error: {e}"));
                let _ = response_tx.send(result);
            }

            AcpRequest::WriteTextFile {
                request,
                response_tx,
            } => {
                debug!("ACP actor: write_text_file {:?}", request.path);
                let result = self
                    .conn
                    .write_text_file(request)
                    .await
                    .map_err(|e| format!("write_text_file error: {e}"));
                let _ = response_tx.send(result);
            }

            AcpRequest::CreateTerminal {
                request,
                response_tx,
            } => {
                debug!("ACP actor: create_terminal {}", request.command);
                let result = self
                    .conn
                    .create_terminal(request)
                    .await
                    .map_err(|e| format!("create_terminal error: {e}"));
                let _ = response_tx.send(result);
            }

            AcpRequest::WaitForTerminalExit {
                request,
                response_tx,
            } => {
                debug!(
                    "ACP actor: wait_for_terminal_exit {:?}",
                    request.terminal_id
                );
                let result = self
                    .conn
                    .wait_for_terminal_exit(request)
                    .await
                    .map_err(|e| format!("wait_for_terminal_exit error: {e}"));
                let _ = response_tx.send(result);
            }

            AcpRequest::TerminalOutput {
                request,
                response_tx,
            } => {
                debug!("ACP actor: terminal_output {:?}", request.terminal_id);
                let result = self
                    .conn
                    .terminal_output(request)
                    .await
                    .map_err(|e| format!("terminal_output error: {e}"));
                let _ = response_tx.send(result);
            }

            AcpRequest::ReleaseTerminal {
                request,
                response_tx,
            } => {
                debug!("ACP actor: release_terminal {:?}", request.terminal_id);
                let result = self
                    .conn
                    .release_terminal(request)
                    .await
                    .map(|_| ()) // Convert response to ()
                    .map_err(|e| format!("release_terminal error: {e}"));
                let _ = response_tx.send(result);
            }

            AcpRequest::SessionNotification {
                notification,
                response_tx,
            } => {
                debug!("ACP actor: session_notification");
                let result = self
                    .conn
                    .session_notification(notification)
                    .await
                    .map_err(|e| format!("session_notification error: {e}"));
                let _ = response_tx.send(result);
            }

            AcpRequest::ExtNotification {
                notification,
                response_tx,
            } => {
                debug!("ACP actor: ext_notification {}", notification.method);
                let result = self
                    .conn
                    .ext_notification(notification)
                    .await
                    .map_err(|e| format!("ext_notification error: {e}"));
                let _ = response_tx.send(result);
            }
        }
    }
}

/// Handle to communicate with the ACP actor
#[derive(Clone, Debug)]
pub struct AcpActorHandle {
    request_tx: mpsc::UnboundedSender<AcpRequest>,
}

impl AcpActorHandle {
    pub fn new(request_tx: mpsc::UnboundedSender<AcpRequest>) -> Self {
        Self { request_tx }
    }

    pub async fn read_text_file(
        &self,
        request: acp::ReadTextFileRequest,
    ) -> Result<acp::ReadTextFileResponse, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::ReadTextFile {
                request,
                response_tx,
            })
            .map_err(|_| "ACP actor channel closed")?;
        response_rx.await.map_err(|_| "Response channel closed")?
    }

    pub async fn write_text_file(
        &self,
        request: acp::WriteTextFileRequest,
    ) -> Result<acp::WriteTextFileResponse, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::WriteTextFile {
                request,
                response_tx,
            })
            .map_err(|_| "ACP actor channel closed")?;
        response_rx.await.map_err(|_| "Response channel closed")?
    }

    pub async fn create_terminal(
        &self,
        request: acp::CreateTerminalRequest,
    ) -> Result<acp::CreateTerminalResponse, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::CreateTerminal {
                request,
                response_tx,
            })
            .map_err(|_| "ACP actor channel closed")?;
        response_rx.await.map_err(|_| "Response channel closed")?
    }

    pub async fn wait_for_terminal_exit(
        &self,
        request: acp::WaitForTerminalExitRequest,
    ) -> Result<acp::WaitForTerminalExitResponse, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::WaitForTerminalExit {
                request,
                response_tx,
            })
            .map_err(|_| "ACP actor channel closed")?;
        response_rx.await.map_err(|_| "Response channel closed")?
    }

    pub async fn terminal_output(
        &self,
        request: acp::TerminalOutputRequest,
    ) -> Result<acp::TerminalOutputResponse, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::TerminalOutput {
                request,
                response_tx,
            })
            .map_err(|_| "ACP actor channel closed")?;
        response_rx.await.map_err(|_| "Response channel closed")?
    }

    pub async fn release_terminal(
        &self,
        request: acp::ReleaseTerminalRequest,
    ) -> Result<(), String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::ReleaseTerminal {
                request,
                response_tx,
            })
            .map_err(|_| "ACP actor channel closed")?;
        response_rx.await.map_err(|_| "Response channel closed")?
    }

    pub async fn send_session_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> Result<(), String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::SessionNotification {
                notification,
                response_tx,
            })
            .map_err(|_| "ACP actor channel closed")?;
        response_rx.await.map_err(|_| "Response channel closed")?
    }

    pub async fn send_ext_notification(
        &self,
        notification: acp::ExtNotification,
    ) -> Result<(), String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::ExtNotification {
                notification,
                response_tx,
            })
            .map_err(|_| "ACP actor channel closed")?;
        response_rx.await.map_err(|_| "Response channel closed")?
    }
}

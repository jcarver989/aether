use crate::lsp_config::get_config_for_language;
use crate::lsp_manager::{LspErrorInfo, LspHandle, LspKey, LspManagerHandle, LspOp, LspResult};
use crate::protocol::{
    DaemonRequest, DaemonResponse, LspErrorResponse, LspRequest, LspResponse, ProtocolError,
    read_frame, write_frame,
};
use std::sync::Arc;
use tokio::io::{ReadHalf, WriteHalf, split};
use tokio::net::UnixStream;
use tokio::spawn;
use tokio::sync::mpsc;

/// Handle a client connection
pub async fn handle_client(
    stream: UnixStream,
    lsp_manager: LspManagerHandle,
    client_id: uuid::Uuid,
) {
    let (reader, writer) = split(stream);
    let (response_tx, response_rx) = mpsc::channel::<DaemonResponse>(100);
    let writer_handle = spawn(run_writer(writer, response_rx));
    run_reader(reader, lsp_manager, client_id, response_tx).await;
    let _ = writer_handle.await;
}

/// Write responses to client
async fn run_writer(
    mut writer: WriteHalf<UnixStream>,
    mut response_rx: mpsc::Receiver<DaemonResponse>,
) {
    while let Some(response) = response_rx.recv().await {
        if let Err(e) = write_frame(&mut writer, &response).await {
            tracing::debug!("Error writing response: {}", e);
            break;
        }
    }
}

/// Read requests from client
async fn run_reader(
    mut reader: ReadHalf<UnixStream>,
    lsp_manager: LspManagerHandle,
    client_id: uuid::Uuid,
    response_tx: mpsc::Sender<DaemonResponse>,
) {
    tracing::debug!("Client connected: {}", client_id);
    let mut lsp_handle: Option<Arc<LspHandle>> = None;

    loop {
        let request: Option<DaemonRequest> = match read_frame(&mut reader).await {
            Ok(Some(req)) => Some(req),
            Ok(None) => {
                tracing::debug!("Client {} disconnected (EOF)", client_id);
                break;
            }
            Err(e) => {
                tracing::debug!("Error reading from client {}: {}", client_id, e);
                break;
            }
        };

        match request {
            Some(DaemonRequest::Ping) => {
                let _ = response_tx.send(DaemonResponse::Pong).await;
            }

            Some(DaemonRequest::Disconnect) => {
                tracing::debug!("Client {} disconnected gracefully", client_id);
                break;
            }

            Some(DaemonRequest::Initialize(init)) => {
                let config = match get_config_for_language(init.language) {
                    Some(c) => c,
                    None => {
                        let _ = response_tx
                            .send(DaemonResponse::Error(ProtocolError::new(format!(
                                "No LSP configured for language: {:?}",
                                init.language
                            ))))
                            .await;
                        continue;
                    }
                };

                let key = LspKey {
                    workspace_root: init.workspace_root.clone(),
                    language: init.language.as_str().to_string(),
                };

                match lsp_manager
                    .get_or_spawn(key, &config.command, &config.args)
                    .await
                {
                    Ok(handle) => {
                        lsp_handle = Some(handle);
                        let _ = response_tx.send(DaemonResponse::Initialized).await;
                    }
                    Err(e) => {
                        let _ = response_tx
                            .send(DaemonResponse::Error(ProtocolError::new(e.to_string())))
                            .await;
                    }
                }
            }

            Some(DaemonRequest::LspRequest(request)) => {
                let client_id = request.client_id();
                let Some(ref handle) = lsp_handle else {
                    let _ = response_tx
                        .send(DaemonResponse::Error(ProtocolError::with_client_id(
                            "Not initialized",
                            client_id,
                        )))
                        .await;
                    continue;
                };

                let response = handle_lsp_request(handle, request).await;
                let _ = response_tx.send(response).await;
            }

            Some(DaemonRequest::LspNotification(notif)) => {
                if let Some(ref handle) = lsp_handle {
                    handle.send_notification(notif).await;
                }
            }

            None => {}
        }
    }
}

/// Handle an LSP request
async fn handle_lsp_request(handle: &LspHandle, request: LspRequest) -> DaemonResponse {
    let client_id = request.client_id();
    let op = match request {
        LspRequest::GotoDefinition { params, .. } => LspOp::GotoDefinition(params),
        LspRequest::GotoImplementation { params, .. } => LspOp::GotoImplementation(params),
        LspRequest::FindReferences { params, .. } => LspOp::FindReferences(params),
        LspRequest::Hover { params, .. } => LspOp::Hover(params),
        LspRequest::WorkspaceSymbol { params, .. } => LspOp::WorkspaceSymbol(params),
        LspRequest::DocumentSymbol { params, .. } => LspOp::DocumentSymbol(params),
        LspRequest::PrepareCallHierarchy { params, .. } => LspOp::PrepareCallHierarchy(params),
        LspRequest::IncomingCalls { params, .. } => LspOp::IncomingCalls(params),
        LspRequest::OutgoingCalls { params, .. } => LspOp::OutgoingCalls(params),
        LspRequest::GetDiagnostics { uri, .. } => {
            return DaemonResponse::LspResponse(LspResponse::GetDiagnostics {
                client_id,
                result: Ok(handle.get_diagnostics(uri.as_ref()).await),
            });
        }
    };

    match handle.request(op).await {
        Ok(result) => {
            let response = match result {
                LspResult::GotoDefinition(r) => LspResponse::GotoDefinition {
                    client_id,
                    result: r.map_err(Into::into),
                },
                LspResult::GotoImplementation(r) => LspResponse::GotoImplementation {
                    client_id,
                    result: r.map_err(Into::into),
                },
                LspResult::FindReferences(r) => LspResponse::FindReferences {
                    client_id,
                    result: r.map_err(Into::into),
                },
                LspResult::Hover(r) => LspResponse::Hover {
                    client_id,
                    result: r.map_err(Into::into),
                },
                LspResult::WorkspaceSymbol(r) => LspResponse::WorkspaceSymbol {
                    client_id,
                    result: r.map_err(Into::into),
                },
                LspResult::DocumentSymbol(r) => LspResponse::DocumentSymbol {
                    client_id,
                    result: r.map_err(Into::into),
                },
                LspResult::PrepareCallHierarchy(r) => LspResponse::PrepareCallHierarchy {
                    client_id,
                    result: r.map_err(Into::into),
                },
                LspResult::IncomingCalls(r) => LspResponse::IncomingCalls {
                    client_id,
                    result: r.map_err(Into::into),
                },
                LspResult::OutgoingCalls(r) => LspResponse::OutgoingCalls {
                    client_id,
                    result: r.map_err(Into::into),
                },
            };
            DaemonResponse::LspResponse(response)
        }
        Err(e) => DaemonResponse::Error(ProtocolError::with_client_id(e.to_string(), client_id)),
    }
}

impl From<LspErrorInfo> for LspErrorResponse {
    fn from(e: LspErrorInfo) -> Self {
        Self {
            code: e.code,
            message: e.message,
        }
    }
}

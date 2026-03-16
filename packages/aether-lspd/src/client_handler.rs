use crate::lsp_config::get_config_for_language;
use crate::lsp_manager::{EnsureDocumentOpenOutcome, LspHandle, LspKey, LspManager};
use crate::protocol::{
    DaemonRequest, DaemonResponse, LspErrorResponse, ProtocolError, extract_document_uri,
    read_frame, write_frame,
};
use std::sync::Arc;
use tokio::io::{ReadHalf, WriteHalf, split};
use tokio::net::UnixStream;
use tokio::spawn;
use tokio::sync::mpsc;

/// LSP error code for "content modified" — a transient error returned by
/// language servers (e.g. rust-analyzer) when a request arrives while the
/// server is still reprocessing after a content change.
const LSP_CONTENT_MODIFIED: i32 = -32801;

/// Maximum number of retries for transient LSP errors.
const TRANSIENT_RETRY_LIMIT: u32 = 3;

/// Delay between retries for transient LSP errors.
const TRANSIENT_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

/// Handle a client connection
#[tracing::instrument(skip(stream, lsp_manager), fields(%client_id))]
pub async fn handle_client(stream: UnixStream, lsp_manager: LspManager, client_id: uuid::Uuid) {
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
    lsp_manager: LspManager,
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
                match try_initialize(&lsp_manager, init).await {
                    Ok(handle) => {
                        lsp_handle = Some(handle);
                        let _ = response_tx.send(DaemonResponse::Initialized).await;
                    }
                    Err(e) => {
                        let _ = response_tx.send(DaemonResponse::Error(e)).await;
                    }
                }
            }

            Some(DaemonRequest::LspCall {
                client_id,
                method,
                params,
            }) => {
                let Some(ref handle) = lsp_handle else {
                    let _ = send_not_initialized(client_id, &response_tx).await;
                    continue;
                };

                tracing::debug!(client_id, %method, "LspCall request");

                let opened_uri = if let Some(uri) = extract_document_uri(&method, &params) {
                    tracing::debug!(uri = %uri.as_str(), %method, "Auto-opening document for LspCall");
                    let _ = handle.ensure_document_open(&uri).await;
                    Some(uri)
                } else {
                    None
                };

                let result =
                    request_with_retry(handle, &method, params, TRANSIENT_RETRY_LIMIT).await;

                // Release the document so the file watcher resumes control.
                if let Some(uri) = opened_uri {
                    tracing::debug!(uri = %uri.as_str(), "Closing document after LspCall");
                    handle.close_document(&uri).await;
                }

                let _ = response_tx
                    .send(DaemonResponse::LspResult { client_id, result })
                    .await;
            }

            Some(DaemonRequest::GetDiagnostics { client_id, uri }) => {
                let Some(ref handle) = lsp_handle else {
                    let _ = send_not_initialized(client_id, &response_tx).await;
                    continue;
                };

                let value = get_diagnostics_and_cleanup(handle, uri).await;
                let _ = response_tx
                    .send(DaemonResponse::LspResult {
                        client_id,
                        result: Ok(value),
                    })
                    .await;
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

/// Sync documents, fetch diagnostics, and close synced documents.
async fn get_diagnostics_and_cleanup(
    handle: &LspHandle,
    uri: Option<lsp_types::Uri>,
) -> serde_json::Value {
    let mode = if uri.is_some() {
        "single-file"
    } else {
        "all-files"
    };
    tracing::info!(
        mode,
        uri = uri.as_ref().map_or("<all>", |u| u.as_str()),
        "GetDiagnostics request"
    );

    // Sync documents before reading the cache so we return
    // fresh diagnostics even when the file has changed on disk.
    let synced_uris: Vec<lsp_types::Uri>;

    if let Some(ref document_uri) = uri {
        // ── Single-file mode ──
        let version_before = handle.ensure_document_open(document_uri).await;
        tracing::debug!(
            uri = %document_uri.as_str(),
            ?version_before,
            "Synced document for diagnostics"
        );

        if let Some(version_before) = version_before {
            handle
                .wait_for_fresh_diagnostics(version_before, std::time::Duration::from_secs(5))
                .await;
        }

        synced_uris = vec![document_uri.clone()];
    } else {
        // ── All-files mode ──
        // Re-sync every URI in the diagnostics cache so that
        // external edits (or MCP edits followed by didClose) are
        // picked up before we return the cache.
        let cached = handle.cached_uris().await;
        tracing::debug!(
            cached_count = cached.len(),
            "All-files mode: syncing cached URIs"
        );

        let mut min_version: Option<u64> = None;
        let mut opened = Vec::new();
        let mut unchanged_count = 0usize;
        let mut sync_failures = 0usize;
        let mut failed_uris = Vec::new();
        for cached_uri in &cached {
            match handle.ensure_document_open_with_outcome(cached_uri).await {
                EnsureDocumentOpenOutcome::Synced(ver) => {
                    min_version = Some(min_version.map_or(ver, |m: u64| m.min(ver)));
                    opened.push(cached_uri.clone());
                }
                EnsureDocumentOpenOutcome::Unchanged => {
                    unchanged_count += 1;
                }
                EnsureDocumentOpenOutcome::Failed => {
                    sync_failures += 1;
                    failed_uris.push(cached_uri.clone());
                }
            }
        }
        tracing::debug!(
            cached_count = cached.len(),
            synced_count = opened.len(),
            unchanged_count,
            sync_failures,
            "All-files mode: sync outcomes"
        );
        if !failed_uris.is_empty() {
            tracing::debug!(
                pruned_uris = failed_uris.len(),
                "All-files mode: pruning unreadable/stale known URIs"
            );
            handle.forget_known_uris(&failed_uris).await;
        }

        if let Some(version_before) = min_version {
            tracing::debug!(
                version_before,
                synced_count = opened.len(),
                "All-files mode: waiting for fresh diagnostics"
            );
            handle
                .wait_for_fresh_diagnostics(version_before, std::time::Duration::from_secs(5))
                .await;
        }

        synced_uris = opened;
    }

    let diagnostics = handle.get_diagnostics(uri.as_ref()).await;
    let diag_count: usize = diagnostics.iter().map(|p| p.diagnostics.len()).sum();
    tracing::info!(
        mode,
        files = diagnostics.len(),
        total_diagnostics = diag_count,
        "Returning diagnostics"
    );

    // Release all synced documents back to file-watcher control.
    for synced_uri in &synced_uris {
        handle.close_document(synced_uri).await;
    }

    serde_json::to_value(&diagnostics).unwrap_or_default()
}

async fn send_not_initialized(
    client_id: i64,
    tx: &mpsc::Sender<DaemonResponse>,
) -> Result<(), mpsc::error::SendError<DaemonResponse>> {
    tx.send(DaemonResponse::Error(ProtocolError::with_client_id(
        "Not initialized",
        client_id,
    )))
    .await
}

/// Try to initialize an LSP server for the given request.
async fn try_initialize(
    lsp_manager: &LspManager,
    init: crate::protocol::InitializeRequest,
) -> Result<Arc<LspHandle>, ProtocolError> {
    let config = get_config_for_language(init.language).ok_or_else(|| {
        ProtocolError::new(format!(
            "No LSP configured for language: {:?}",
            init.language
        ))
    })?;

    let workspace_root = init
        .workspace_root
        .canonicalize()
        .unwrap_or(init.workspace_root);

    let key = LspKey {
        workspace_root,
        language: init.language,
    };

    lsp_manager
        .get_or_spawn(key, &config.command, &config.args)
        .await
        .map_err(|e| ProtocolError::new(e.to_string()))
}

/// Retry an LSP request when the server returns a transient "content modified"
/// error, giving the language server time to finish reprocessing.
async fn request_with_retry(
    handle: &LspHandle,
    method: &str,
    params: serde_json::Value,
    max_retries: u32,
) -> Result<serde_json::Value, LspErrorResponse> {
    let mut last_err = None;
    for attempt in 0..=max_retries {
        match handle.request_raw(method, params.clone()).await {
            Ok(value) => return Ok(value),
            Err(err) if err.code == LSP_CONTENT_MODIFIED && attempt < max_retries => {
                tracing::debug!(
                    %method,
                    attempt = attempt + 1,
                    max_retries,
                    "Retrying after transient 'content modified' error"
                );
                last_err = Some(err);
                tokio::time::sleep(TRANSIENT_RETRY_DELAY).await;
            }
            Err(err) => return Err(err),
        }
    }
    Err(last_err.unwrap())
}

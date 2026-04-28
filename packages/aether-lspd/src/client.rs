use std::collections::HashMap;
use std::io;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem, CallHierarchyOutgoingCall,
    CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams, DocumentSymbolParams, DocumentSymbolResponse,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, Location, PartialResultParams, Position,
    PublishDiagnosticsParams, ReferenceContext, ReferenceParams, RenameParams, SymbolInformation,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::{Mutex, oneshot};

use crate::language_catalog::LanguageId;
use crate::protocol::{DaemonRequest, DaemonResponse, InitializeRequest, read_frame, write_frame};
use crate::socket_path::{ensure_socket_dir, log_file_path};

#[doc = include_str!("docs/client_error.md")]
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Failed to connect to daemon: {0}")]
    ConnectionFailed(#[source] io::Error),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Daemon error: {0}")]
    DaemonError(String),

    #[error("LSP error (code={code}): {message}")]
    LspError { code: i32, message: String },

    #[error("Failed to spawn daemon: {0}")]
    SpawnFailed(#[source] io::Error),

    #[error("Timeout waiting for daemon to start")]
    SpawnTimeout,

    #[error("Daemon binary not found: {0}")]
    DaemonBinaryNotFound(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
}

pub type ClientResult<T> = std::result::Result<T, ClientError>;

#[doc = include_str!("docs/client.md")]
pub struct LspClient {
    writer: Mutex<WriteHalf<UnixStream>>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>>,
    next_id: AtomicI64,
    reader_task: tokio::task::JoinHandle<()>,
}

impl LspClient {
    pub async fn connect(workspace_root: &Path, language: LanguageId) -> ClientResult<Self> {
        let socket_path = ensure_socket_dir(workspace_root, language).map_err(ClientError::Io)?;

        match UnixStream::connect(&socket_path).await {
            Ok(stream) => {
                return Self::from_stream(stream, workspace_root, language).await;
            }
            Err(err) if err.kind() == ErrorKind::ConnectionRefused || err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(ClientError::ConnectionFailed(err)),
        }

        spawn_daemon(&socket_path).await?;
        let stream = UnixStream::connect(&socket_path).await.map_err(ClientError::ConnectionFailed)?;
        Self::from_stream(stream, workspace_root, language).await
    }

    pub async fn goto_definition(&self, uri: Uri, line: u32, character: u32) -> ClientResult<GotoDefinitionResponse> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("textDocument/definition", &params, || GotoDefinitionResponse::Array(vec![])).await
    }

    pub async fn goto_implementation(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
    ) -> ClientResult<GotoDefinitionResponse> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("textDocument/implementation", &params, || GotoDefinitionResponse::Array(vec![])).await
    }

    pub async fn find_references(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> ClientResult<Vec<Location>> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext { include_declaration },
        };
        self.call("textDocument/references", &params, Vec::new).await
    }

    pub async fn hover(&self, uri: Uri, line: u32, character: u32) -> ClientResult<Option<Hover>> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.call("textDocument/hover", &params, || None).await
    }

    pub async fn workspace_symbol(&self, query: String) -> ClientResult<Vec<SymbolInformation>> {
        let params = WorkspaceSymbolParams {
            query,
            partial_result_params: PartialResultParams::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.call("workspace/symbol", &params, Vec::new).await
    }

    pub async fn document_symbol(&self, uri: Uri) -> ClientResult<DocumentSymbolResponse> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("textDocument/documentSymbol", &params, || DocumentSymbolResponse::Flat(vec![])).await
    }

    pub async fn prepare_call_hierarchy(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
    ) -> ClientResult<Vec<CallHierarchyItem>> {
        let params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.call("textDocument/prepareCallHierarchy", &params, Vec::new).await
    }

    pub async fn incoming_calls(&self, item: CallHierarchyItem) -> ClientResult<Vec<CallHierarchyIncomingCall>> {
        let params = CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("callHierarchy/incomingCalls", &params, Vec::new).await
    }

    pub async fn outgoing_calls(&self, item: CallHierarchyItem) -> ClientResult<Vec<CallHierarchyOutgoingCall>> {
        let params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("callHierarchy/outgoingCalls", &params, Vec::new).await
    }

    pub async fn rename(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
        new_name: String,
    ) -> ClientResult<Option<WorkspaceEdit>> {
        let params = RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            new_name,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.call("textDocument/rename", &params, || None).await
    }

    pub async fn get_diagnostics(&self, uri: Option<Uri>) -> ClientResult<Vec<PublishDiagnosticsParams>> {
        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::GetDiagnostics { client_id, uri };

        self.send_and_await(request, client_id)
            .await
            .and_then(|value| serde_json::from_value(value).map_err(|err| ClientError::ProtocolError(err.to_string())))
    }

    pub async fn queue_diagnostic_refresh(&self, uri: Uri) -> ClientResult<()> {
        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::QueueDiagnosticRefresh { client_id, uri };
        self.send_and_await(request, client_id).await.map(|_| ())
    }

    pub async fn disconnect(self) -> ClientResult<()> {
        let request = DaemonRequest::Disconnect;
        let mut writer = self.writer.lock().await;
        write_frame(&mut *writer, &request).await.map_err(ClientError::Io)
    }

    pub async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: &P,
        default: impl FnOnce() -> R,
    ) -> ClientResult<R> {
        let params_value = serde_json::to_value(params).map_err(|err| ClientError::ProtocolError(err.to_string()))?;

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspCall { client_id, method: method.to_string(), params: params_value };

        let value = self.send_and_await(request, client_id).await?;

        if value.is_null() {
            Ok(default())
        } else {
            serde_json::from_value(value).map_err(|err| ClientError::ProtocolError(format!("Parse error: {err}")))
        }
    }
}

impl LspClient {
    async fn from_stream(stream: UnixStream, workspace_root: &Path, language: LanguageId) -> ClientResult<Self> {
        let (mut reader, mut writer) = tokio::io::split(stream);

        let initialize =
            DaemonRequest::Initialize(InitializeRequest { workspace_root: workspace_root.to_path_buf(), language });

        write_frame(&mut writer, &initialize).await.map_err(ClientError::Io)?;

        let response: Option<DaemonResponse> = read_frame(&mut reader).await.map_err(ClientError::Io)?;

        match response {
            Some(DaemonResponse::Initialized) => {}
            Some(DaemonResponse::Error(err)) => {
                return Err(ClientError::InitializationFailed(err.message));
            }
            Some(_) => {
                return Err(ClientError::ProtocolError("Unexpected response to Initialize".into()));
            }
            None => {
                return Err(ClientError::ProtocolError("Connection closed during initialization".into()));
            }
        }

        let pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>> = Arc::new(Mutex::new(HashMap::new()));

        let pending_clone = Arc::clone(&pending);
        let reader_task = tokio::spawn(async move {
            run_reader(reader, pending_clone).await;
        });

        Ok(Self { writer: Mutex::new(writer), pending, next_id: AtomicI64::new(1), reader_task })
    }

    async fn send_and_await(&self, request: DaemonRequest, client_id: i64) -> ClientResult<Value> {
        let (response_tx, response_rx) = oneshot::channel();

        {
            let mut pending = self.pending.lock().await;
            pending.insert(client_id, response_tx);
        }

        let write_result = {
            let mut writer = self.writer.lock().await;
            write_frame(&mut *writer, &request).await
        };

        if let Err(err) = write_result {
            self.pending.lock().await.remove(&client_id);
            return Err(ClientError::Io(err));
        }

        response_rx.await.map_err(|_| ClientError::ProtocolError("Response channel closed".into()))?
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

type PendingResult = Result<Value, ClientError>;

type ReadyCheck = futures::future::BoxFuture<'static, bool>;

#[derive(Clone, Debug)]
struct DaemonProcessStatus {
    success: bool,
    display: String,
}

impl From<ExitStatus> for DaemonProcessStatus {
    fn from(status: ExitStatus) -> Self {
        Self { success: status.success(), display: status.to_string() }
    }
}

trait DaemonChild {
    fn try_wait(&mut self) -> io::Result<Option<DaemonProcessStatus>>;
    fn kill(&mut self) -> futures::future::BoxFuture<'_, io::Result<()>>;
    fn wait(&mut self) -> futures::future::BoxFuture<'_, io::Result<DaemonProcessStatus>>;
}

impl DaemonChild for tokio::process::Child {
    fn try_wait(&mut self) -> io::Result<Option<DaemonProcessStatus>> {
        tokio::process::Child::try_wait(self).map(|status| status.map(DaemonProcessStatus::from))
    }

    fn kill(&mut self) -> futures::future::BoxFuture<'_, io::Result<()>> {
        Box::pin(async move { tokio::process::Child::kill(self).await })
    }

    fn wait(&mut self) -> futures::future::BoxFuture<'_, io::Result<DaemonProcessStatus>> {
        Box::pin(async move { tokio::process::Child::wait(self).await.map(DaemonProcessStatus::from) })
    }
}

async fn run_reader(
    mut reader: ReadHalf<UnixStream>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>>,
) {
    loop {
        let response: Option<DaemonResponse> = match read_frame(&mut reader).await {
            Ok(Some(response)) => Some(response),
            Ok(None) => break,
            Err(err) => {
                tracing::debug!(%err, "Error reading daemon response");
                break;
            }
        };

        match response {
            Some(DaemonResponse::LspResult { client_id, result }) => {
                let mut pending = pending.lock().await;
                if let Some(tx) = pending.remove(&client_id) {
                    let value_result =
                        result.map_err(|err| ClientError::LspError { code: err.code, message: err.message });
                    let _ = tx.send(value_result);
                }
            }
            Some(DaemonResponse::Error(err)) => {
                if let Some(client_id) = err.client_id {
                    let mut pending = pending.lock().await;
                    if let Some(tx) = pending.remove(&client_id) {
                        let _ = tx.send(Err(ClientError::DaemonError(err.message)));
                    }
                }
            }
            _ => {}
        }
    }

    let mut pending = pending.lock().await;
    for (_, tx) in pending.drain() {
        let _ = tx.send(Err(ClientError::ProtocolError("Daemon disconnected".into())));
    }
}

async fn wait_for_daemon_ready<C, F>(
    child: &mut C,
    mut is_ready: F,
    attempts: usize,
    poll_interval: Duration,
) -> ClientResult<()>
where
    C: DaemonChild,
    F: FnMut() -> ReadyCheck,
{
    for _ in 0..attempts {
        match child.try_wait() {
            Ok(Some(status)) if !status.success => {
                return Err(ClientError::SpawnFailed(std::io::Error::other(format!(
                    "Daemon exited with status: {}",
                    status.display
                ))));
            }
            Ok(Some(_) | None) => {}
            Err(err) => return Err(ClientError::SpawnFailed(err)),
        }

        tokio::time::sleep(poll_interval).await;
        if is_ready().await {
            return Ok(());
        }
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
    Err(ClientError::SpawnTimeout)
}

fn spawn_daemon_reaper<C>(mut child: C) -> tokio::task::JoinHandle<()>
where
    C: DaemonChild + Send + 'static,
{
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => {
                tracing::debug!(status = %status.display, success = status.success, "aether-lspd launcher reaped");
            }
            Err(err) => tracing::warn!(%err, "Failed to reap aether-lspd launcher"),
        }
    })
}

async fn spawn_daemon(socket_path: &Path) -> ClientResult<()> {
    let (binary, subcommand) = find_daemon_binary()?;
    let log_file = log_file_path(socket_path);

    let mut cmd = Command::new(&binary);
    if let Some(sub) = subcommand {
        cmd.arg(sub);
    }
    cmd.arg("--socket")
        .arg(socket_path)
        .arg("--log-file")
        .arg(&log_file)
        .arg("--log-level")
        .arg("debug")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().map_err(ClientError::SpawnFailed)?;
    let ready_socket = socket_path.to_path_buf();

    wait_for_daemon_ready(
        &mut child,
        move || {
            let socket_path = ready_socket.clone();
            Box::pin(async move { UnixStream::connect(socket_path).await.is_ok() })
        },
        50,
        Duration::from_millis(100),
    )
    .await?;

    spawn_daemon_reaper(child);
    Ok(())
}

fn find_daemon_binary() -> ClientResult<(PathBuf, Option<&'static str>)> {
    let exe = std::env::current_exe().ok();
    let exe_dir = exe.as_deref().and_then(|p| p.parent());

    let standalone_candidates = [
        exe_dir.map(|dir| dir.join("aether-lspd")),
        exe_dir.and_then(|dir| dir.parent()).map(|dir| dir.join("aether-lspd")),
        which_aether_lspd(),
        Some(PathBuf::from("target/debug/aether-lspd")),
        Some(PathBuf::from("target/release/aether-lspd")),
        Some(PathBuf::from("../../target/debug/aether-lspd")),
        Some(PathBuf::from("../../target/release/aether-lspd")),
    ];

    for candidate in standalone_candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok((candidate, None));
        }
    }

    if let Some(exe) = exe {
        return Ok((exe, Some("lspd")));
    }

    Err(ClientError::DaemonBinaryNotFound("aether-lspd not found".into()))
}

fn which_aether_lspd() -> Option<PathBuf> {
    std::env::var_os("PATH")
        .and_then(|paths| std::env::split_paths(&paths).map(|path| path.join("aether-lspd")).find(|path| path.exists()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::sync::Notify;

    struct FakeDaemonChild {
        failed_status: Option<DaemonProcessStatus>,
        kill_calls: Arc<AtomicUsize>,
        wait_calls: Arc<AtomicUsize>,
        waited: Arc<Notify>,
    }

    impl FakeDaemonChild {
        fn running() -> Self {
            Self {
                failed_status: None,
                kill_calls: Arc::new(AtomicUsize::new(0)),
                wait_calls: Arc::new(AtomicUsize::new(0)),
                waited: Arc::new(Notify::new()),
            }
        }
    }

    impl DaemonChild for FakeDaemonChild {
        fn try_wait(&mut self) -> std::io::Result<Option<DaemonProcessStatus>> {
            Ok(self.failed_status.clone())
        }

        fn kill(&mut self) -> futures::future::BoxFuture<'_, std::io::Result<()>> {
            let kill_calls = Arc::clone(&self.kill_calls);
            Box::pin(async move {
                kill_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }

        fn wait(&mut self) -> futures::future::BoxFuture<'_, std::io::Result<DaemonProcessStatus>> {
            let wait_calls = Arc::clone(&self.wait_calls);
            let waited = Arc::clone(&self.waited);
            Box::pin(async move {
                wait_calls.fetch_add(1, Ordering::SeqCst);
                waited.notify_waiters();
                Ok(DaemonProcessStatus { success: true, display: "exit status: 0".to_string() })
            })
        }
    }

    #[tokio::test]
    async fn reaper_waits_for_spawned_daemon_launcher() {
        let child = FakeDaemonChild::running();
        let wait_calls = Arc::clone(&child.wait_calls);
        let waited = Arc::clone(&child.waited);

        let handle = spawn_daemon_reaper(child);
        waited.notified().await;
        handle.await.expect("reaper task should complete");

        assert_eq!(wait_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn startup_timeout_kills_and_reaps_launcher() {
        let mut child = FakeDaemonChild::running();
        let kill_calls = Arc::clone(&child.kill_calls);
        let wait_calls = Arc::clone(&child.wait_calls);
        let ready = Arc::new(AtomicBool::new(false));
        let ready_check = Arc::clone(&ready);

        let result = wait_for_daemon_ready(
            &mut child,
            move || {
                let ready = Arc::clone(&ready_check);
                Box::pin(async move { ready.load(Ordering::SeqCst) })
            },
            1,
            Duration::ZERO,
        )
        .await;

        assert!(matches!(result, Err(ClientError::SpawnTimeout)));
        assert_eq!(kill_calls.load(Ordering::SeqCst), 1);
        assert_eq!(wait_calls.load(Ordering::SeqCst), 1);
    }
}

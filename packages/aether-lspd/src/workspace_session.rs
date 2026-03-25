use crate::diagnostics_store::DiagnosticsStore;
use crate::document_coordinator::{DocumentCoordinator, SyncPlan};
use crate::process_transport::{ProcessTransport, TransportEvent};
use crate::protocol::LspNotification;
use ignore::WalkBuilder;
use lsp_types::notification::{DidChangeWatchedFiles, Notification};
use lsp_types::{DidChangeWatchedFilesParams, PublishDiagnosticsParams, Uri};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify, mpsc};

const DIAGNOSTICS_TIMEOUT: Duration = Duration::from_secs(10);
const BACKGROUND_REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct WorkspaceSession {
    transport: ProcessTransport,
    documents: DocumentCoordinator,
    diagnostics: DiagnosticsStore,
    refresh: BackgroundRefresh,
}

#[derive(Clone)]
struct BackgroundRefresh {
    state: Arc<Mutex<BackgroundRefreshState>>,
    wake: Arc<Notify>,
    progress: Arc<Notify>,
}

struct BackgroundRefreshState {
    pending_queue: VecDeque<Uri>,
    pending_set: HashSet<Uri>,
    scheduled_generation: u64,
    completed_generation: u64,
    bootstrap_in_progress: bool,
    active: bool,
    shutdown: bool,
}

impl WorkspaceSession {
    pub(crate) fn spawn(
        workspace_root: &Path,
        command: &str,
        args: &[String],
        supported_extensions: HashSet<String>,
    ) -> crate::DaemonResult<Self> {
        let (transport, event_rx) = ProcessTransport::spawn(workspace_root, command, args)?;
        let documents = DocumentCoordinator::new();
        let diagnostics = DiagnosticsStore::new();
        let refresh =
            BackgroundRefresh::spawn(transport.clone(), documents.clone(), diagnostics.clone());

        let session = Self {
            transport,
            documents,
            diagnostics,
            refresh,
        };

        let supported_extensions = Arc::new(supported_extensions);

        tokio::spawn(run_session_events(
            session.transport.clone(),
            session.documents.clone(),
            session.diagnostics.clone(),
            session.refresh.clone(),
            Arc::clone(&supported_extensions),
            event_rx,
        ));

        tokio::spawn(bootstrap_workspace_refresh(
            workspace_root.to_path_buf(),
            supported_extensions,
            session.refresh.clone(),
        ));

        Ok(session)
    }

    pub(crate) async fn request_raw(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, crate::LspErrorResponse> {
        self.transport.request_raw(method, params).await
    }

    pub(crate) async fn queue_diagnostic_refresh(&self, uri: Uri) {
        self.refresh.enqueue(vec![uri]).await;
    }

    pub(crate) async fn ensure_document_open(&self, uri: &Uri) -> Option<u64> {
        match self.documents.prepare_request_document(uri).await {
            SyncPlan::Sync(notifications) => {
                let version_before = self.diagnostics.current_version();
                for notification in notifications {
                    self.transport.send_notification(notification).await;
                }
                Some(version_before)
            }
            SyncPlan::Unchanged | SyncPlan::Failed => None,
        }
    }

    pub(crate) async fn close_document(&self, uri: &Uri) {
        if let Some(notification) = self.documents.release_request_document(uri).await {
            self.transport.send_notification(notification).await;
        }
    }

    pub(crate) async fn get_diagnostics(&self, uri: Option<&Uri>) -> Vec<PublishDiagnosticsParams> {
        self.sync_documents_for_diagnostics(uri).await;
        self.diagnostics.get(uri).await
    }

    pub(crate) async fn shutdown(&self) {
        self.refresh.shutdown().await;
        self.transport.shutdown().await;
    }

    async fn sync_documents_for_diagnostics(&self, uri: Option<&Uri>) {
        if let Some(uri) = uri {
            let version_before = self.ensure_document_open(uri).await;
            if let Some(version_before) = version_before {
                self.diagnostics
                    .wait_for_fresh(version_before, DIAGNOSTICS_TIMEOUT)
                    .await;
            } else {
                self.refresh
                    .wait_for_current_generation(DIAGNOSTICS_TIMEOUT)
                    .await;
            }
            self.close_document(uri).await;
            return;
        }

        self.refresh
            .wait_for_current_generation(BACKGROUND_REFRESH_TIMEOUT)
            .await;
    }
}

impl BackgroundRefresh {
    fn spawn(
        transport: ProcessTransport,
        documents: DocumentCoordinator,
        diagnostics: DiagnosticsStore,
    ) -> Self {
        let refresh = Self {
            state: Arc::new(Mutex::new(BackgroundRefreshState {
                pending_queue: VecDeque::new(),
                pending_set: HashSet::new(),
                scheduled_generation: 1,
                completed_generation: 0,
                bootstrap_in_progress: true,
                active: false,
                shutdown: false,
            })),
            wake: Arc::new(Notify::new()),
            progress: Arc::new(Notify::new()),
        };

        tokio::spawn(run_background_refresh_worker(
            transport,
            documents,
            diagnostics,
            refresh.clone(),
        ));

        refresh
    }

    async fn enqueue(&self, uris: Vec<Uri>) {
        if uris.is_empty() {
            return;
        }

        let mut state = self.state.lock().await;
        let mut added = false;
        for uri in uris {
            if state.pending_set.insert(uri.clone()) {
                state.pending_queue.push_back(uri);
                added = true;
            }
        }

        if !added {
            return;
        }

        if !state.bootstrap_in_progress {
            state.scheduled_generation += 1;
        }
        drop(state);
        self.wake.notify_one();
    }

    async fn complete_bootstrap(&self) {
        let mut should_notify = false;
        {
            let mut state = self.state.lock().await;
            state.bootstrap_in_progress = false;
            if !state.active && state.pending_queue.is_empty() {
                state.completed_generation = state.scheduled_generation;
                should_notify = true;
            }
        }

        if should_notify {
            self.progress.notify_waiters();
        }
        self.wake.notify_one();
    }

    async fn wait_for_current_generation(&self, timeout: Duration) {
        let target = self.state.lock().await.scheduled_generation;
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            {
                let state = self.state.lock().await;
                if state.completed_generation >= target {
                    return;
                }
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return;
            }

            tokio::select! {
                () = self.progress.notified() => {}
                () = tokio::time::sleep(remaining) => return,
            }
        }
    }

    async fn next_uri(&self) -> Option<Uri> {
        let mut should_notify = false;
        let next = {
            let mut state = self.state.lock().await;
            if state.shutdown {
                return None;
            }

            if let Some(uri) = state.pending_queue.pop_front() {
                state.pending_set.remove(&uri);
                state.active = true;
                Some(uri)
            } else {
                state.active = false;
                if !state.bootstrap_in_progress
                    && state.completed_generation != state.scheduled_generation
                {
                    state.completed_generation = state.scheduled_generation;
                    should_notify = true;
                }
                None
            }
        };

        if should_notify {
            self.progress.notify_waiters();
        }

        next
    }

    async fn shutdown(&self) {
        {
            let mut state = self.state.lock().await;
            state.shutdown = true;
        }
        self.wake.notify_waiters();
        self.progress.notify_waiters();
    }
}

async fn run_background_refresh_worker(
    transport: ProcessTransport,
    documents: DocumentCoordinator,
    diagnostics: DiagnosticsStore,
    refresh: BackgroundRefresh,
) {
    loop {
        let Some(uri) = refresh.next_uri().await else {
            let wake = refresh.wake.notified();
            let should_shutdown = refresh.state.lock().await.shutdown;
            if should_shutdown {
                break;
            }
            wake.await;
            continue;
        };

        refresh_uri(&transport, &documents, &diagnostics, &uri).await;
    }
}

async fn refresh_uri(
    transport: &ProcessTransport,
    documents: &DocumentCoordinator,
    diagnostics: &DiagnosticsStore,
    uri: &Uri,
) {
    let version_before = match documents.prepare_request_document(uri).await {
        SyncPlan::Sync(notifications) => {
            let version_before = diagnostics.current_version();
            for notification in notifications {
                transport.send_notification(notification).await;
            }
            Some(version_before)
        }
        SyncPlan::Unchanged => None,
        SyncPlan::Failed => {
            documents.forget_uris(std::slice::from_ref(uri)).await;
            diagnostics.forget(std::slice::from_ref(uri)).await;
            return;
        }
    };

    if let Some(version_before) = version_before {
        diagnostics
            .wait_for_fresh(version_before, DIAGNOSTICS_TIMEOUT)
            .await;
    }

    if let Some(notification) = documents.release_request_document(uri).await {
        transport.send_notification(notification).await;
    }
}

async fn bootstrap_workspace_refresh(
    workspace_root: PathBuf,
    supported_extensions: Arc<HashSet<String>>,
    refresh: BackgroundRefresh,
) {
    let uris = if supported_extensions.is_empty() {
        Vec::new()
    } else {
        tokio::task::spawn_blocking(move || {
            let mut builder = WalkBuilder::new(&workspace_root);
            builder.standard_filters(true);

            let mut uris = Vec::new();
            for entry in builder.build() {
                let Ok(entry) = entry else {
                    continue;
                };
                if !entry
                    .file_type()
                    .is_some_and(|file_type| file_type.is_file())
                {
                    continue;
                }
                if !path_is_supported(entry.path(), supported_extensions.as_ref()) {
                    continue;
                }
                if let Ok(uri) = crate::path_to_uri(entry.path()) {
                    uris.push(uri);
                }
            }
            uris
        })
        .await
        .unwrap_or_default()
    };

    refresh.enqueue(uris).await;
    refresh.complete_bootstrap().await;
}

async fn run_session_events(
    transport: ProcessTransport,
    documents: DocumentCoordinator,
    diagnostics: DiagnosticsStore,
    refresh: BackgroundRefresh,
    supported_extensions: Arc<HashSet<String>>,
    mut event_rx: mpsc::Receiver<TransportEvent>,
) {
    while let Some(event) = event_rx.recv().await {
        match event {
            TransportEvent::PublishedDiagnostics(params) => {
                documents
                    .remember_uris(std::slice::from_ref(&params.uri))
                    .await;
                diagnostics.publish(params).await;
            }
            TransportEvent::FileWatcherBatch(batch) => {
                let mut remembered = batch.discovered_uris.clone();
                remembered.extend(
                    batch
                        .forwarded_changes
                        .iter()
                        .map(|change| change.uri.clone()),
                );
                documents.remember_uris(&remembered).await;

                let filtered = documents
                    .filter_watcher_changes(batch.forwarded_changes)
                    .await;
                let discovered =
                    filter_supported_uris(batch.discovered_uris, supported_extensions.as_ref());

                let mut refresh_uris = filter_supported_uris(
                    filtered.iter().map(|change| change.uri.clone()).collect(),
                    supported_extensions.as_ref(),
                );
                refresh_uris.extend(discovered);
                refresh.enqueue(refresh_uris).await;

                if filtered.is_empty() {
                    continue;
                }

                let params = DidChangeWatchedFilesParams { changes: filtered };
                if let Ok(value) = serde_json::to_value(&params) {
                    transport
                        .send_notification(LspNotification {
                            method: DidChangeWatchedFiles::METHOD.to_string(),
                            params: value,
                        })
                        .await;
                }
            }
            TransportEvent::Closed => break,
        }
    }
}

fn filter_supported_uris(uris: Vec<Uri>, supported_extensions: &HashSet<String>) -> Vec<Uri> {
    uris.into_iter()
        .filter(|uri| uri_is_supported(uri, supported_extensions))
        .collect()
}

fn uri_is_supported(uri: &Uri, supported_extensions: &HashSet<String>) -> bool {
    let path = crate::uri_to_path(uri);
    path_is_supported(Path::new(&path), supported_extensions)
}

fn path_is_supported(path: &Path, supported_extensions: &HashSet<String>) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| supported_extensions.contains(ext))
}

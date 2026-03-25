use lsp_types::{PublishDiagnosticsParams, Uri};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, RwLock};

const DIAGNOSTICS_SETTLE_DURATION: Duration = Duration::from_millis(600);

#[derive(Clone, Default)]
pub(crate) struct DiagnosticsStore {
    state: Arc<RwLock<HashMap<Uri, PublishDiagnosticsParams>>>,
    notify: Arc<Notify>,
    version: Arc<AtomicU64>,
}

impl DiagnosticsStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn publish(&self, diagnostics: PublishDiagnosticsParams) {
        self.state
            .write()
            .await
            .insert(diagnostics.uri.clone(), diagnostics);
        self.version.fetch_add(1, Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    pub(crate) fn current_version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    pub(crate) async fn get(&self, uri: Option<&Uri>) -> Vec<PublishDiagnosticsParams> {
        let state = self.state.read().await;
        if let Some(uri) = uri {
            state.get(uri).cloned().into_iter().collect()
        } else {
            state.values().cloned().collect()
        }
    }

    pub(crate) async fn forget(&self, uris: &[Uri]) {
        if uris.is_empty() {
            return;
        }

        let mut state = self.state.write().await;
        for uri in uris {
            state.remove(uri);
        }
    }

    pub(crate) async fn wait_for_fresh(&self, version_before: u64, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let current_version = self.current_version();
            if current_version != version_before {
                break;
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return;
            }

            tokio::select! {
                () = self.notify.notified() => {}
                () = tokio::time::sleep(remaining) => return,
            }
        }

        let mut last_version = self.current_version();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return;
            }

            let settle_wait = DIAGNOSTICS_SETTLE_DURATION.min(remaining);
            tokio::select! {
                () = self.notify.notified() => {
                    let new_version = self.current_version();
                    if new_version != last_version {
                        last_version = new_version;
                    }
                }
                () = tokio::time::sleep(settle_wait) => {
                    let final_version = self.current_version();
                    if final_version == last_version {
                        return;
                    }
                    last_version = final_version;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Diagnostic, Position, Range};

    fn diagnostics(uri: &str, message: &str) -> PublishDiagnosticsParams {
        PublishDiagnosticsParams {
            uri: uri.parse().unwrap(),
            diagnostics: vec![Diagnostic {
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 1),
                },
                message: message.to_string(),
                ..Default::default()
            }],
            version: None,
        }
    }

    #[tokio::test]
    async fn wait_for_fresh_waits_for_settle_window() {
        let store = DiagnosticsStore::new();
        let version_before = store.current_version();
        let publish_store = store.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            publish_store
                .publish(diagnostics("file:///test.rs", "first"))
                .await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            publish_store
                .publish(diagnostics("file:///test.rs", "second"))
                .await;
        });

        let start = tokio::time::Instant::now();
        store
            .wait_for_fresh(version_before, Duration::from_secs(2))
            .await;

        assert!(
            start.elapsed() >= Duration::from_millis(600),
            "store should wait through the settle window"
        );
        let diags = store.get(None).await;
        assert_eq!(diags[0].diagnostics[0].message, "second");
    }

    #[tokio::test]
    async fn forget_removes_cached_diagnostics() {
        let store = DiagnosticsStore::new();
        let uri: Uri = "file:///test.rs".parse().unwrap();
        store.publish(diagnostics(uri.as_str(), "error")).await;
        store.forget(std::slice::from_ref(&uri)).await;
        assert!(store.get(Some(&uri)).await.is_empty());
    }
}

use crate::language_catalog::LanguageId;
use crate::protocol::LspNotification;
use crate::uri::uri_to_path;
use lsp_types::notification::{DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument, Notification};
use lsp_types::{
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams, FileEvent,
    TextDocumentIdentifier, TextDocumentItem, Uri,
};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::read_to_string;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub(crate) struct DocumentCoordinator {
    state: Arc<RwLock<DocumentState>>,
}

#[derive(Default)]
struct DocumentState {
    documents: HashMap<Uri, DocumentEntry>,
}

#[derive(Clone, Copy, Debug, Default)]
struct DocumentEntry {
    is_open: bool,
    content_hash: Option<u64>,
}

pub(crate) enum SyncPlan {
    Sync(Vec<LspNotification>),
    Unchanged,
    Failed,
}

impl DocumentCoordinator {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn remember_uris(&self, uris: &[Uri]) {
        if uris.is_empty() {
            return;
        }

        let mut state = self.state.write().await;
        for uri in uris {
            state.documents.entry(uri.clone()).or_default();
        }
    }

    pub(crate) async fn forget_uris(&self, uris: &[Uri]) {
        if uris.is_empty() {
            return;
        }

        let mut state = self.state.write().await;
        for uri in uris {
            state.documents.remove(uri);
        }
    }

    pub(crate) async fn prepare_request_document(&self, uri: &Uri) -> SyncPlan {
        let file_path = uri_to_path(uri);
        let Ok(content) = read_to_string(&file_path).await else {
            return SyncPlan::Failed;
        };

        let content_hash = hash_content(&content);
        let mut state = self.state.write().await;

        let entry = state.documents.entry(uri.clone()).or_default();
        if entry.is_open {
            if entry.content_hash == Some(content_hash) {
                SyncPlan::Unchanged
            } else {
                entry.content_hash = Some(content_hash);
                SyncPlan::Sync(reopen_notifications(uri, &file_path, content))
            }
        } else {
            entry.is_open = true;
            entry.content_hash = Some(content_hash);
            SyncPlan::Sync(open_and_save_notifications(uri, &file_path, content))
        }
    }

    pub(crate) async fn release_request_document(&self, uri: &Uri) -> Option<LspNotification> {
        let mut state = self.state.write().await;
        let entry = state.documents.get_mut(uri)?;
        if !entry.is_open {
            return None;
        }
        entry.is_open = false;
        Some(close_notification(uri))
    }

    pub(crate) async fn filter_watcher_changes(&self, changes: Vec<FileEvent>) -> Vec<FileEvent> {
        let state = self.state.read().await;
        changes
            .into_iter()
            .filter(|change| state.documents.get(&change.uri).is_none_or(|entry| !entry.is_open))
            .collect()
    }
}

fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn open_and_save_notifications(uri: &Uri, file_path: &str, content: String) -> Vec<LspNotification> {
    vec![open_notification(uri, file_path, 1, content), save_notification(uri)]
}

fn reopen_notifications(uri: &Uri, file_path: &str, content: String) -> Vec<LspNotification> {
    vec![close_notification(uri), open_notification(uri, file_path, 1, content), save_notification(uri)]
}

fn open_notification(uri: &Uri, file_path: &str, version: i32, content: String) -> LspNotification {
    let language_id = LanguageId::from_path(Path::new(file_path));
    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: language_id.as_str().to_string(),
            version,
            text: content,
        },
    };
    LspNotification { method: DidOpenTextDocument::METHOD.to_string(), params: serde_json::to_value(&params).unwrap() }
}

fn save_notification(uri: &Uri) -> LspNotification {
    let params = DidSaveTextDocumentParams { text_document: TextDocumentIdentifier { uri: uri.clone() }, text: None };
    LspNotification { method: DidSaveTextDocument::METHOD.to_string(), params: serde_json::to_value(&params).unwrap() }
}

fn close_notification(uri: &Uri) -> LspNotification {
    let params = DidCloseTextDocumentParams { text_document: TextDocumentIdentifier { uri: uri.clone() } };
    LspNotification { method: DidCloseTextDocument::METHOD.to_string(), params: serde_json::to_value(&params).unwrap() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::notification::DidCloseTextDocument;
    use tempfile::TempDir;

    fn uri_for(path: &Path) -> Uri {
        crate::path_to_uri(path).unwrap()
    }

    #[tokio::test]
    async fn documents_auto_close_after_release() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("main.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        let uri = uri_for(&path);
        let coordinator = DocumentCoordinator::new();

        assert!(matches!(coordinator.prepare_request_document(&uri).await, SyncPlan::Sync(_)));

        let close = coordinator.release_request_document(&uri).await.unwrap();
        assert_eq!(close.method, DidCloseTextDocument::METHOD);
        assert!(coordinator.release_request_document(&uri).await.is_none());
    }
}

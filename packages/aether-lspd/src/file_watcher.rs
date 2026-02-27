use crate::uri::path_to_uri;
use globset::{Glob, GlobSet, GlobSetBuilder};
use lsp_types::{FileChangeType, FileEvent, FileSystemWatcher, Uri, WatchKind};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Sleep;

/// Messages sent from the handle to the actor.
enum FileWatcherMsg {
    Register {
        id: String,
        watchers: Vec<FileSystemWatcher>,
    },
    Unregister {
        id: String,
    },
}

#[derive(Debug)]
pub(crate) struct FileWatcherBatch {
    pub(crate) forwarded_changes: Vec<FileEvent>,
    pub(crate) discovered_uris: Vec<Uri>,
}

/// Handle that sends messages to the [`FileWatcherActor`] via an mpsc channel.
pub struct FileWatcherHandle {
    msg_tx: mpsc::Sender<FileWatcherMsg>,
    _task: JoinHandle<()>,
}

impl FileWatcherHandle {
    /// Spawn the actor task and return a handle to it.
    pub fn spawn(workspace_root: PathBuf, event_tx: mpsc::Sender<FileWatcherBatch>) -> Self {
        let (msg_tx, msg_rx) = mpsc::channel(64);
        let (bridge_tx, bridge_rx) = mpsc::channel::<Event>(256);
        let watcher = match create_watcher(&workspace_root, bridge_tx) {
            Ok(w) => {
                tracing::debug!("Started file watcher on {}", workspace_root.display());
                Some(w)
            }
            Err(e) => {
                tracing::error!("Failed to start file watcher: {e}");
                None
            }
        };
        let canonical_workspace_root = std::fs::canonicalize(&workspace_root)
            .ok()
            .filter(|canonical| canonical != &workspace_root);
        if let Some(canonical) = &canonical_workspace_root {
            tracing::debug!(
                workspace_root = %workspace_root.display(),
                canonical_workspace_root = %canonical.display(),
                "File watcher: using canonical workspace root for path matching"
            );
        }

        let actor = FileWatcherActor {
            _watcher: watcher,
            workspace_root,
            canonical_workspace_root,
            event_tx,
            msg_rx,
            bridge_rx,
            forwarded_pending: HashMap::new(),
            discovered_pending: HashMap::new(),
            registrations: HashMap::new(),
            glob_set: GlobSet::empty(),
            watch_kinds: Vec::new(),
        };

        let task = tokio::spawn(actor.run());

        Self {
            msg_tx,
            _task: task,
        }
    }

    /// Register file watchers for a `workspace/didChangeWatchedFiles` registration.
    pub fn register_watchers(&self, id: String, watchers: Vec<FileSystemWatcher>) {
        let _ = self
            .msg_tx
            .try_send(FileWatcherMsg::Register { id, watchers });
    }

    /// Unregister file watchers for a given registration ID.
    pub fn unregister(&self, id: String) {
        let _ = self.msg_tx.try_send(FileWatcherMsg::Unregister { id });
    }
}

/// Owns all file-watcher state and processes messages sequentially in a spawned task.
struct FileWatcherActor {
    _watcher: Option<RecommendedWatcher>,
    workspace_root: PathBuf,
    canonical_workspace_root: Option<PathBuf>,
    event_tx: mpsc::Sender<FileWatcherBatch>,
    msg_rx: mpsc::Receiver<FileWatcherMsg>,
    bridge_rx: mpsc::Receiver<notify::Event>,
    forwarded_pending: HashMap<String, (Uri, FileChangeType)>,
    discovered_pending: HashMap<String, Uri>,
    registrations: HashMap<String, Vec<FileSystemWatcher>>,
    glob_set: GlobSet,
    watch_kinds: Vec<WatchKind>,
}

impl FileWatcherActor {
    async fn run(mut self) {
        let debounce = Duration::from_millis(200);
        let mut timer: Option<Pin<Box<Sleep>>> = None;

        loop {
            tokio::select! {
                msg = self.msg_rx.recv() => {
                    let Some(msg) = msg else { break };
                    match msg {
                        FileWatcherMsg::Register { id, watchers } => {
                            self.registrations.insert(id, watchers);
                            self.rebuild_glob_state();
                        }
                        FileWatcherMsg::Unregister { id } => {
                            if self.registrations.remove(&id).is_some() {
                                tracing::debug!("Unregistered file watcher {id}");
                                self.rebuild_glob_state();
                            }
                        }
                    }
                }
                Some(ev) = self.bridge_rx.recv() => {
                    self.accumulate_event(&ev);
                    if self.has_pending() {
                        timer = Some(Box::pin(tokio::time::sleep(debounce)));
                    }
                }
                () = async { match &mut timer { Some(t) => t.as_mut().await, None => std::future::pending().await } } => {
                    timer = None;
                    self.flush_pending().await;
                }
            }
        }

        self.flush_pending().await;
    }

    fn rebuild_glob_state(&mut self) {
        let all_watchers: Vec<&FileSystemWatcher> =
            self.registrations.values().flat_map(|v| v.iter()).collect();

        let (glob_set, watch_kinds) =
            build_glob_set(&all_watchers).unwrap_or_else(|| (GlobSet::empty(), Vec::new()));

        self.glob_set = glob_set;
        self.watch_kinds = watch_kinds;
    }

    fn accumulate_event(&mut self, ev: &Event) {
        for path in &ev.paths {
            let rel_from_workspace = path.strip_prefix(&self.workspace_root).ok();
            let rel_from_canonical = self
                .canonical_workspace_root
                .as_ref()
                .and_then(|root| path.strip_prefix(root).ok());
            let rel = rel_from_workspace
                .or(rel_from_canonical)
                .unwrap_or(path.as_path());
            let within_workspace = rel_from_workspace.is_some() || rel_from_canonical.is_some();

            // Try relative path first, fall back to absolute
            let matches = self.glob_set.matches(rel);
            let matches = if matches.is_empty() {
                self.glob_set.matches(path)
            } else {
                matches
            };
            if matches.is_empty() {
                // Keep implicit URI discovery enabled even after watcher registration.
                // Some LSPs register narrow globs (e.g. Cargo files) that do not include
                // all source-file edits needed by all-files diagnostics.
                if !should_track_implicit_path(ev.kind, within_workspace, rel, path) {
                    tracing::trace!(
                        path = %path.display(),
                        kind = ?ev.kind,
                        "File event: implicit discovery ignored path"
                    );
                    continue;
                }
                if map_event_kind(
                    ev.kind,
                    WatchKind::Create | WatchKind::Change | WatchKind::Delete,
                )
                .is_none()
                {
                    continue;
                }
                let Ok(uri) = path_to_uri(path) else {
                    continue;
                };
                let key = uri.to_string();
                tracing::trace!(
                    path = %path.display(),
                    kind = ?ev.kind,
                    "File event: no glob match, tracking URI discovery only"
                );
                self.discovered_pending.insert(key, uri);
                continue;
            }

            let effective_kinds = matches
                .iter()
                .fold(WatchKind::empty(), |acc, &i| acc | self.watch_kinds[i]);

            let Some(change_type) = map_event_kind(ev.kind, effective_kinds) else {
                continue;
            };

            let Ok(uri) = path_to_uri(path) else {
                continue;
            };
            let key = uri.to_string();
            tracing::debug!(
                path = %path.display(),
                change_type = ?change_type,
                "File event: accumulated for debounced dispatch"
            );
            self.forwarded_pending.insert(key, (uri, change_type));
        }
    }

    async fn flush_pending(&mut self) {
        if !self.has_pending() {
            return;
        }

        let mut forwarded_changes: Vec<FileEvent> = self
            .forwarded_pending
            .drain()
            .map(|(_, (uri, typ))| FileEvent { uri, typ })
            .collect();
        forwarded_changes.sort_by(|a, b| a.uri.as_str().cmp(b.uri.as_str()));

        let mut discovered_uris: Vec<Uri> = self
            .discovered_pending
            .drain()
            .map(|(_, uri)| uri)
            .collect();
        discovered_uris.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        tracing::debug!(
            forwarded_changes = forwarded_changes.len(),
            discovered_uris = discovered_uris.len(),
            "Sending file watcher batch"
        );

        let batch = FileWatcherBatch {
            forwarded_changes,
            discovered_uris,
        };
        if self.event_tx.send(batch).await.is_err() {
            tracing::debug!("File watcher channel closed");
        }
    }

    fn has_pending(&self) -> bool {
        !self.forwarded_pending.is_empty() || !self.discovered_pending.is_empty()
    }
}

/// Create the OS file watcher that bridges notify events into an mpsc channel.
fn create_watcher(
    workspace_root: &Path,
    tx: mpsc::Sender<Event>,
) -> Result<RecommendedWatcher, notify::Error> {
    let mut watcher = RecommendedWatcher::new(
        move |event: Result<Event, notify::Error>| match event {
            Ok(e) => {
                let _ = tx.blocking_send(e);
            }
            Err(e) => {
                tracing::debug!("File watcher error: {e}");
            }
        },
        Config::default(),
    )?;

    watcher.watch(workspace_root, RecursiveMode::Recursive)?;
    Ok(watcher)
}

/// Build a `GlobSet` paired with per-glob `WatchKind` flags.
///
/// The returned `Vec<WatchKind>` is index-aligned with the globs added to the `GlobSet`,
/// so `GlobSet::matches(path)` indices can be used to look up the corresponding kind.
fn build_glob_set(watchers: &[&FileSystemWatcher]) -> Option<(GlobSet, Vec<WatchKind>)> {
    let mut builder = GlobSetBuilder::new();
    let mut kinds = Vec::new();

    for w in watchers {
        let pattern = match &w.glob_pattern {
            lsp_types::GlobPattern::String(s) => s.as_str(),
            lsp_types::GlobPattern::Relative(rp) => rp.pattern.as_str(),
        };

        match Glob::new(pattern) {
            Ok(g) => {
                builder.add(g);
                kinds.push(
                    w.kind
                        .unwrap_or(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
                );
            }
            Err(e) => {
                tracing::warn!("Invalid glob pattern '{pattern}': {e}");
            }
        }
    }

    builder
        .build()
        .inspect_err(|e| tracing::error!("Failed to build glob set: {e}"))
        .ok()
        .filter(|gs| !gs.is_empty())
        .map(|gs| (gs, kinds))
}

/// Map a `notify::EventKind` to an LSP `FileChangeType`, filtered by requested `WatchKind`.
fn map_event_kind(kind: EventKind, watch_kinds: WatchKind) -> Option<FileChangeType> {
    match kind {
        EventKind::Create(_) if watch_kinds.contains(WatchKind::Create) => {
            Some(FileChangeType::CREATED)
        }
        EventKind::Modify(_) if watch_kinds.contains(WatchKind::Change) => {
            Some(FileChangeType::CHANGED)
        }
        EventKind::Remove(_) if watch_kinds.contains(WatchKind::Delete) => {
            Some(FileChangeType::DELETED)
        }
        _ => None,
    }
}

fn should_track_implicit_path(
    kind: EventKind,
    within_workspace: bool,
    rel_path: &Path,
    absolute_path: &Path,
) -> bool {
    // Only discover implicit URIs inside the watched workspace root.
    if !within_workspace {
        return false;
    }

    // Skip noisy build/system directories that can generate thousands of irrelevant
    // changes and binary blobs (which can't be opened as text documents anyway).
    for component in rel_path.components() {
        let std::path::Component::Normal(name) = component else {
            continue;
        };
        let component = name.to_string_lossy();
        if matches!(
            component.as_ref(),
            ".git" | "node_modules" | ".next" | "dist" | "build" | "target"
        ) {
            return false;
        }
    }

    // Ignore directory changes for implicit discovery; these can't be synced as
    // text documents and only add noise to known_uris.
    if !matches!(kind, EventKind::Remove(_)) && absolute_path.is_dir() {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_watcher(pattern: &str, kind: Option<WatchKind>) -> FileSystemWatcher {
        FileSystemWatcher {
            glob_pattern: lsp_types::GlobPattern::String(pattern.into()),
            kind,
        }
    }

    fn test_actor(workspace_root: &str) -> (FileWatcherActor, mpsc::Receiver<FileWatcherBatch>) {
        let (event_tx, event_rx) = mpsc::channel(8);
        let (_, msg_rx) = mpsc::channel(8);
        let (_, bridge_rx) = mpsc::channel(8);

        (
            FileWatcherActor {
                _watcher: None,
                workspace_root: PathBuf::from(workspace_root),
                canonical_workspace_root: None,
                event_tx,
                msg_rx,
                bridge_rx,
                forwarded_pending: HashMap::new(),
                discovered_pending: HashMap::new(),
                registrations: HashMap::new(),
                glob_set: GlobSet::empty(),
                watch_kinds: Vec::new(),
            },
            event_rx,
        )
    }

    #[test]
    fn test_build_glob_set() {
        let watchers = [make_watcher(
            "**/*.rs",
            Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
        )];
        let refs: Vec<&FileSystemWatcher> = watchers.iter().collect();

        let (gs, kinds) = build_glob_set(&refs).unwrap();
        assert!(gs.is_match("src/main.rs"));
        assert!(!gs.is_match("src/main.py"));
        assert_eq!(kinds.len(), 1);
        assert_eq!(
            kinds[0],
            WatchKind::Create | WatchKind::Change | WatchKind::Delete
        );
    }

    #[test]
    fn test_build_glob_set_no_valid_patterns() {
        let watchers = [make_watcher("[invalid", None)];
        let refs: Vec<&FileSystemWatcher> = watchers.iter().collect();

        assert!(build_glob_set(&refs).is_none());
    }

    #[test]
    fn test_build_glob_set_empty() {
        assert!(build_glob_set(&[]).is_none());
    }

    #[test]
    fn test_build_glob_set_preserves_per_watcher_kinds() {
        let watchers = [
            make_watcher("**/*.rs", Some(WatchKind::Create)),
            make_watcher("**/*.json", Some(WatchKind::Delete)),
        ];
        let refs: Vec<&FileSystemWatcher> = watchers.iter().collect();

        let (gs, kinds) = build_glob_set(&refs).unwrap();

        // .rs matches index 0 only → Create
        let rs_matches = gs.matches("src/main.rs");
        let rs_kind = rs_matches
            .iter()
            .fold(WatchKind::empty(), |acc, &i| acc | kinds[i]);
        assert_eq!(rs_kind, WatchKind::Create);

        // .json matches index 1 only → Delete
        let json_matches = gs.matches("config.json");
        let json_kind = json_matches
            .iter()
            .fold(WatchKind::empty(), |acc, &i| acc | kinds[i]);
        assert_eq!(json_kind, WatchKind::Delete);

        // A Delete event on a .rs file should NOT pass the kind filter
        assert!(
            map_event_kind(EventKind::Remove(notify::event::RemoveKind::File), rs_kind).is_none()
        );

        // A Create event on a .json file should NOT pass the kind filter
        assert!(
            map_event_kind(
                EventKind::Create(notify::event::CreateKind::File),
                json_kind
            )
            .is_none()
        );
    }

    #[test]
    fn test_build_glob_set_skips_invalid_keeps_indices_aligned() {
        let watchers = [
            make_watcher("**/*.rs", Some(WatchKind::Create)),
            make_watcher("[invalid", Some(WatchKind::Change)),
            make_watcher("**/*.json", Some(WatchKind::Delete)),
        ];
        let refs: Vec<&FileSystemWatcher> = watchers.iter().collect();

        let (gs, kinds) = build_glob_set(&refs).unwrap();

        // Invalid glob was skipped, so we should have exactly 2 entries
        assert_eq!(kinds.len(), 2);
        assert_eq!(kinds[0], WatchKind::Create);
        assert_eq!(kinds[1], WatchKind::Delete);

        // .rs matches index 0 → Create
        let rs_matches = gs.matches("lib.rs");
        assert_eq!(rs_matches, vec![0]);

        // .json matches index 1 → Delete
        let json_matches = gs.matches("data.json");
        assert_eq!(json_matches, vec![1]);
    }

    #[test]
    fn test_map_event_kind() {
        let all_kinds = WatchKind::Create | WatchKind::Change | WatchKind::Delete;

        assert_eq!(
            map_event_kind(
                EventKind::Create(notify::event::CreateKind::File),
                all_kinds
            ),
            Some(FileChangeType::CREATED)
        );
        assert_eq!(
            map_event_kind(
                EventKind::Modify(notify::event::ModifyKind::Data(
                    notify::event::DataChange::Content
                )),
                all_kinds
            ),
            Some(FileChangeType::CHANGED)
        );
        assert_eq!(
            map_event_kind(
                EventKind::Remove(notify::event::RemoveKind::File),
                all_kinds
            ),
            Some(FileChangeType::DELETED)
        );

        // When change kind not requested, should return None
        assert_eq!(
            map_event_kind(
                EventKind::Create(notify::event::CreateKind::File),
                WatchKind::Change
            ),
            None
        );
    }

    #[test]
    fn test_path_to_uri() {
        let uri = path_to_uri(std::path::Path::new("/home/user/project/src/main.rs")).unwrap();
        assert_eq!(uri.to_string(), "file:///home/user/project/src/main.rs");
    }

    #[test]
    fn test_should_track_implicit_path_skips_target_tree() {
        assert!(!should_track_implicit_path(
            EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content
            )),
            true,
            Path::new("target/debug/incremental/foo.bin"),
            Path::new("/tmp/project/target/debug/incremental/foo.bin")
        ));
    }

    #[test]
    fn test_should_track_implicit_path_allows_targeting_directory() {
        assert!(should_track_implicit_path(
            EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content
            )),
            true,
            Path::new("targeting/main.rs"),
            Path::new("/tmp/project/targeting/main.rs")
        ));
    }

    #[test]
    fn test_should_track_implicit_path_rejects_outside_workspace() {
        assert!(!should_track_implicit_path(
            EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content
            )),
            false,
            Path::new("/tmp/other/main.rs"),
            Path::new("/tmp/other/main.rs")
        ));
    }

    #[test]
    fn test_rebuild_glob_state_combines_registrations() {
        let (mut actor, _) = test_actor("/tmp");

        actor.registrations.insert(
            "reg1".to_owned(),
            vec![make_watcher("**/*.rs", Some(WatchKind::Create))],
        );
        actor.registrations.insert(
            "reg2".to_owned(),
            vec![make_watcher("**/*.json", Some(WatchKind::Delete))],
        );

        actor.rebuild_glob_state();

        assert!(actor.glob_set.is_match("src/main.rs"));
        assert!(actor.glob_set.is_match("config.json"));
        assert!(!actor.glob_set.is_match("readme.md"));
        assert_eq!(actor.watch_kinds.len(), 2);
    }

    #[test]
    fn test_accumulate_event_uses_implicit_mode_without_globs() {
        let (mut actor, _) = test_actor("/tmp/project");

        let ev = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from("/tmp/project/src/main.rs")],
            attrs: notify::event::EventAttributes::new(),
        };

        actor.accumulate_event(&ev);

        assert!(actor.forwarded_pending.is_empty());
        assert_eq!(actor.discovered_pending.len(), 1);
    }

    #[test]
    fn test_accumulate_event_matching_glob_forwards_changes() {
        let (mut actor, _) = test_actor("/tmp/project");
        actor.registrations.insert(
            "reg1".to_owned(),
            vec![make_watcher("**/*.rs", Some(WatchKind::Change))],
        );
        actor.rebuild_glob_state();

        let ev = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from("/tmp/project/src/main.rs")],
            attrs: notify::event::EventAttributes::new(),
        };

        actor.accumulate_event(&ev);

        assert_eq!(actor.forwarded_pending.len(), 1);
        assert!(actor.discovered_pending.is_empty());
        let (_, (_, change_type)) = actor.forwarded_pending.iter().next().unwrap();
        assert_eq!(*change_type, FileChangeType::CHANGED);
    }

    #[test]
    fn test_accumulate_event_tracks_non_matching_paths() {
        let (mut actor, _) = test_actor("/tmp/project");
        actor.registrations.insert(
            "reg1".to_owned(),
            vec![make_watcher("**/*.rs", Some(WatchKind::Change))],
        );
        actor.rebuild_glob_state();

        let ev = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from("/tmp/project/src/main.py")],
            attrs: notify::event::EventAttributes::new(),
        };

        actor.accumulate_event(&ev);

        assert!(actor.forwarded_pending.is_empty());
        assert_eq!(actor.discovered_pending.len(), 1);
    }

    #[test]
    fn test_accumulate_event_ignores_target_noise_in_implicit_mode() {
        let (mut actor, _) = test_actor("/tmp/project");

        let ev = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from(
                "/tmp/project/target/debug/incremental/foo/dep-graph.bin",
            )],
            attrs: notify::event::EventAttributes::new(),
        };

        actor.accumulate_event(&ev);

        assert!(actor.forwarded_pending.is_empty());
        assert!(actor.discovered_pending.is_empty());
    }

    #[test]
    fn test_accumulate_event_ignores_outside_workspace_in_implicit_mode() {
        let (mut actor, _) = test_actor("/tmp/project");

        let ev = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from("/tmp/other/outside.rs")],
            attrs: notify::event::EventAttributes::new(),
        };

        actor.accumulate_event(&ev);

        assert!(actor.forwarded_pending.is_empty());
        assert!(actor.discovered_pending.is_empty());
    }

    #[tokio::test]
    async fn test_flush_pending_only_discovered_emits_track_only_batch() {
        let (mut actor, mut event_rx) = test_actor("/tmp/project");

        let ev = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from("/tmp/project/src/main.rs")],
            attrs: notify::event::EventAttributes::new(),
        };
        actor.accumulate_event(&ev);

        actor.flush_pending().await;

        let batch = event_rx.recv().await.expect("expected file watcher batch");
        assert!(batch.forwarded_changes.is_empty());
        assert_eq!(batch.discovered_uris.len(), 1);
    }

    #[tokio::test]
    async fn test_register_and_unregister_via_handle() {
        let (event_tx, _event_rx) = mpsc::channel(64);
        let handle = FileWatcherHandle::spawn(PathBuf::from("/tmp/test"), event_tx);

        // Register two sets
        handle.register_watchers(
            "reg1".to_owned(),
            vec![make_watcher("**/*.rs", Some(WatchKind::Create))],
        );
        handle.register_watchers(
            "reg2".to_owned(),
            vec![make_watcher("**/*.json", Some(WatchKind::Delete))],
        );

        // Unregister one
        handle.unregister("reg1".to_owned());

        // Unregister the other
        handle.unregister("reg2".to_owned());

        // Drop the handle — actor should exit cleanly
        drop(handle);

        // Give the actor time to process and shut down
        tokio::task::yield_now().await;
    }
}

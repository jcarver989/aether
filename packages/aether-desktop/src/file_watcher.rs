//! File system watching with debouncing for git diff updates.
//!
//! Uses the `notify` crate with debouncing to watch for file changes
//! in an agent's working directory.

use crate::error::AetherDesktopError;
use notify::RecommendedWatcher;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

/// Default debounce duration for file system events.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(300);

/// Events emitted by the file watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileWatchEvent {
    /// Files have changed in the watched directory.
    Changed,
    /// An error occurred while watching.
    Error(String),
}

/// A file system watcher that emits debounced change events.
///
/// The watcher filters out `.git` directory changes and temporary files.
pub struct FileWatcher {
    /// Kept alive to maintain the watch. Dropping this stops the watcher.
    #[allow(dead_code)]
    debouncer: Debouncer<RecommendedWatcher>,
    path: PathBuf,
}

impl FileWatcher {
    /// Create a new file watcher for the given path.
    ///
    /// Events are sent through the provided channel after debouncing.
    /// The watcher filters out:
    /// - Changes in `.git/` directories
    /// - Temporary files (ending in `~`, `.swp`, or `.swx`)
    pub fn new(
        path: PathBuf,
        tx: mpsc::UnboundedSender<FileWatchEvent>,
    ) -> Result<Self, AetherDesktopError> {
        let debouncer = new_debouncer(
            DEBOUNCE_DURATION,
            move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                match result {
                    Ok(events) => {
                        let has_relevant_events = events.iter().any(|e| {
                            e.kind == DebouncedEventKind::Any && is_relevant_path(&e.path)
                        });

                        if has_relevant_events {
                            let _ = tx.send(FileWatchEvent::Changed);
                        }
                    }
                    Err(error) => {
                        let _ = tx.send(FileWatchEvent::Error(error.to_string()));
                    }
                }
            },
        )
        .map_err(|e| AetherDesktopError::FileWatcherCreation(e.to_string()))?;

        let mut watcher = Self { debouncer, path };

        watcher.start_watching()?;

        Ok(watcher)
    }

    fn start_watching(&mut self) -> Result<(), AetherDesktopError> {
        use notify::RecursiveMode;

        self.debouncer
            .watcher()
            .watch(&self.path, RecursiveMode::Recursive)
            .map_err(|e| AetherDesktopError::FileWatcherPath(e.to_string()))?;

        Ok(())
    }

    /// Get the path being watched.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Check if a path should trigger a file watch event.
///
/// Filters out `.git` directories and common temporary file patterns.
fn is_relevant_path(path: &Path) -> bool {
    // Skip .git directory
    if path
        .components()
        .any(|c| c.as_os_str() == OsStr::new(".git"))
    {
        return false;
    }

    // Skip temporary files
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        if file_name.ends_with('~') || file_name.ends_with(".swp") || file_name.ends_with(".swx") {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_file_watcher_detects_change() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();

        let _watcher = FileWatcher::new(temp_dir.path().to_path_buf(), tx).unwrap();

        // Give the watcher time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a file
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

        // Wait for the debounced event (300ms debounce + some buffer)
        let result = timeout(Duration::from_secs(2), rx.recv()).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Some(FileWatchEvent::Changed)));
    }

    #[tokio::test]
    async fn test_file_watcher_ignores_git_dir() {
        let temp_dir = TempDir::new().unwrap();

        // Create .git directory
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel();

        let _watcher = FileWatcher::new(temp_dir.path().to_path_buf(), tx).unwrap();

        // Give the watcher time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a file inside .git
        fs::write(git_dir.join("index"), "git data").unwrap();

        // Wait a bit more than the debounce time
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Should not receive any event for .git changes
        let result = rx.try_recv();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_watcher_debounce() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();

        let _watcher = FileWatcher::new(temp_dir.path().to_path_buf(), tx).unwrap();

        // Give the watcher time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Make multiple rapid changes
        for i in 0..5 {
            fs::write(temp_dir.path().join("test.txt"), format!("content {}", i)).unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Wait for debounce to settle
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Should receive only one or two events due to debouncing
        let mut event_count = 0;
        while rx.try_recv().is_ok() {
            event_count += 1;
        }

        // Debouncing should limit the number of events
        assert!(
            event_count <= 2,
            "Expected <= 2 events but got {}",
            event_count
        );
    }

    #[test]
    fn test_is_relevant_path() {
        // Regular files should be relevant
        assert!(is_relevant_path(Path::new("/project/src/main.rs")));
        assert!(is_relevant_path(Path::new("foo/bar.txt")));

        // .git directory should be ignored
        assert!(!is_relevant_path(Path::new("/project/.git/index")));
        assert!(!is_relevant_path(Path::new(".git/objects/pack")));
        assert!(!is_relevant_path(Path::new("/repo/.git/HEAD")));

        // Temporary files should be ignored
        assert!(!is_relevant_path(Path::new("/project/file.txt~")));
        assert!(!is_relevant_path(Path::new("/project/.main.rs.swp")));
        assert!(!is_relevant_path(Path::new("/project/.main.rs.swx")));
    }
}

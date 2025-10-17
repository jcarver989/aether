use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Trait for file system operations.
/// Allows swapping in fakes for testing
pub trait Fs {
    fn write_file(&self, path: &str, content: &str) -> impl Future<Output = Result<(), String>>;
    fn read_file(&self, path: &str) -> impl Future<Output = Result<String, String>>;
    fn list_files(&self) -> impl Future<Output = Result<Vec<String>, String>>;
    fn file_exists(&self, path: &str) -> impl Future<Output = bool>;
}

/// Standard filesystem implementation using std::fs
#[derive(Debug, Clone, Default)]
pub struct StdFileSystem;

impl StdFileSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Fs for StdFileSystem {
    async fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
        fs::write(path, content).map_err(|e| format!("Failed to write file {}: {}", path, e))
    }

    async fn read_file(&self, path: &str) -> Result<String, String> {
        fs::read_to_string(path).map_err(|e| format!("Failed to read file {}: {}", path, e))
    }

    async fn list_files(&self) -> Result<Vec<String>, String> {
        let entries = fs::read_dir(".")
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            if path.is_file() {
                if let Some(path_str) = path.to_str() {
                    files.push(path_str.to_string());
                }
            }
        }
        Ok(files)
    }

    async fn file_exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
}

/// In-memory filesystem for testing
#[derive(Debug, Clone, Default)]
pub struct InMemoryFileSystem {
    files: Arc<Mutex<HashMap<String, String>>>,
}

impl InMemoryFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Fs for InMemoryFileSystem {
    async fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
        let mut files = self.files.lock().await;
        files.insert(path.to_string(), content.to_string());
        Ok(())
    }

    async fn read_file(&self, path: &str) -> Result<String, String> {
        let files = self.files.lock().await;
        files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("File not found: {path}"))
    }

    async fn list_files(&self) -> Result<Vec<String>, String> {
        let files = self.files.lock().await;
        Ok(files.keys().cloned().collect())
    }

    async fn file_exists(&self, path: &str) -> bool {
        let files = self.files.lock().await;
        files.contains_key(path)
    }
}

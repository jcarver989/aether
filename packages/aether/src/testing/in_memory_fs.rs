use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::fs::Fs;

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

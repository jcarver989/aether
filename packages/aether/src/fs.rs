use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

/// File type enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

/// File information returned by list_files_detailed
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub file_type: FileType,
    pub size: u64,
    pub permissions: String,
    pub modified: Option<String>,
}

/// Trait for file system operations.
/// Allows swapping in fakes for testing
pub trait Fs {
    fn write_file(&self, path: &str, content: &str) -> impl Future<Output = io::Result<()>> + Send;
    fn read_file(&self, path: &str) -> impl Future<Output = io::Result<String>> + Send;
    fn list_files(&self) -> impl Future<Output = io::Result<Vec<String>>> + Send;
    fn list_files_detailed(&self, path: &str) -> impl Future<Output = io::Result<Vec<FileInfo>>> + Send;
    fn file_exists(&self, path: &str) -> impl Future<Output = bool> + Send;
    fn create_dir_all(&self, path: &str) -> impl Future<Output = io::Result<()>> + Send;
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
    async fn write_file(&self, path: &str, content: &str) -> io::Result<()> {
        fs::write(path, content)
    }

    async fn read_file(&self, path: &str) -> io::Result<String> {
        fs::read_to_string(path)
    }

    async fn list_files(&self) -> io::Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(".")? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(path_str) = path.to_str() {
                    files.push(path_str.to_string());
                }
            }
        }
        Ok(files)
    }

    async fn list_files_detailed(&self, path: &str) -> io::Result<Vec<FileInfo>> {
        use std::os::unix::fs::PermissionsExt;

        let mut files = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;

            if let Some(name) = entry.file_name().to_str() {
                let file_type = if metadata.is_dir() {
                    FileType::Directory
                } else if metadata.is_symlink() {
                    FileType::Symlink
                } else {
                    FileType::File
                };

                let permissions = format!("{:o}", metadata.permissions().mode() & 0o777);

                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|duration| {
                        let secs = duration.as_secs();
                        format!("{}", secs)
                    });

                files.push(FileInfo {
                    name: name.to_string(),
                    path: path.to_string_lossy().to_string(),
                    file_type,
                    size: metadata.len(),
                    permissions,
                    modified,
                });
            }
        }
        Ok(files)
    }

    async fn file_exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    async fn create_dir_all(&self, path: &str) -> io::Result<()> {
        fs::create_dir_all(path)
    }
}

/// Metadata for in-memory files
#[derive(Debug, Clone)]
struct InMemoryFileMetadata {
    content: String,
    permissions: String,
    modified_timestamp: u64,
}

/// In-memory filesystem for testing
#[derive(Debug, Clone, Default)]
pub struct InMemoryFileSystem {
    files: Arc<Mutex<HashMap<String, InMemoryFileMetadata>>>,
}

impl InMemoryFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Fs for InMemoryFileSystem {
    async fn write_file(&self, path: &str, content: &str) -> io::Result<()> {
        let mut files = self.files.lock().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        files.insert(
            path.to_string(),
            InMemoryFileMetadata {
                content: content.to_string(),
                permissions: "644".to_string(), // Default permissions
                modified_timestamp: now,
            },
        );
        Ok(())
    }

    async fn read_file(&self, path: &str) -> io::Result<String> {
        let files = self.files.lock().await;
        files
            .get(path)
            .map(|metadata| metadata.content.clone())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("File not found: {path}"))
            })
    }

    async fn list_files(&self) -> io::Result<Vec<String>> {
        let files = self.files.lock().await;
        Ok(files.keys().cloned().collect())
    }

    async fn list_files_detailed(&self, path: &str) -> io::Result<Vec<FileInfo>> {
        let files = self.files.lock().await;
        let mut result = Vec::new();

        // Normalize the path to ensure consistent matching
        let normalized_path = if path == "." {
            ""
        } else {
            path.trim_end_matches('/')
        };

        for (file_path, metadata) in files.iter() {
            // Check if file is in the requested directory
            if normalized_path.is_empty() || file_path.starts_with(&format!("{}/", normalized_path)) {
                // Extract the file name from the full path
                let name = file_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(file_path)
                    .to_string();

                result.push(FileInfo {
                    name,
                    path: file_path.clone(),
                    file_type: FileType::File,
                    size: metadata.content.len() as u64,
                    permissions: metadata.permissions.clone(),
                    modified: Some(metadata.modified_timestamp.to_string()),
                });
            }
        }

        Ok(result)
    }

    async fn file_exists(&self, path: &str) -> bool {
        let files = self.files.lock().await;
        files.contains_key(path)
    }

    async fn create_dir_all(&self, _path: &str) -> io::Result<()> {
        // In-memory filesystem doesn't need directory creation
        // Directories are implicit from file paths
        Ok(())
    }
}

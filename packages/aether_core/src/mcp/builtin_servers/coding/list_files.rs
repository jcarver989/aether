use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListFilesArgs {
    /// Directory path to list (defaults to current directory if not provided)
    pub path: Option<String>,
    /// Include hidden files (starting with .)
    pub include_hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub file_type: FileType,
    pub size: u64,
    pub permissions: String,
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFilesResult {
    pub status: String,
    pub directory: String,
    pub files: Vec<FileInfo>,
    pub total_count: usize,
}

pub async fn list_files(args: ListFilesArgs) -> Result<ListFilesResult, String> {
    let target_path = args.path.as_deref().unwrap_or(".");
    let include_hidden = args.include_hidden.unwrap_or(false);

    let path = Path::new(target_path);

    if !path.exists() {
        return Err(format!("Path does not exist: {}", target_path));
    }

    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", target_path));
    }

    let entries = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    let mut files = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("Error reading entry: {}", e))?;
        let file_path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files unless requested
        if !include_hidden && file_name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata()
            .map_err(|e| format!("Failed to read metadata for {}: {}", file_name, e))?;

        let file_type = if metadata.is_dir() {
            FileType::Directory
        } else if metadata.is_symlink() {
            FileType::Symlink
        } else {
            FileType::File
        };

        let permissions = format!("{:o}", metadata.permissions().mode() & 0o777);

        let modified = metadata.modified()
            .ok()
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|duration| {
                let secs = duration.as_secs();
                chrono::DateTime::from_timestamp(secs as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            });

        files.push(FileInfo {
            name: file_name,
            path: file_path.to_string_lossy().to_string(),
            file_type,
            size: metadata.len(),
            permissions,
            modified,
        });
    }

    // Sort by name for consistent output
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let total_count = files.len();
    let canonical_path = path.canonicalize()
        .unwrap_or_else(|_| PathBuf::from(target_path))
        .to_string_lossy()
        .to_string();

    Ok(ListFilesResult {
        status: "success".to_string(),
        directory: canonical_path,
        files,
        total_count,
    })
}
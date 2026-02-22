use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use crate::coding::error::ListFilesError;
use mcp_utils::display_meta::{ToolDisplayMeta, ToolResultMeta, basename};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub file_type: FileType,
    pub size: u64,
    pub permissions: String,
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesResult {
    pub status: String,
    pub directory: String,
    pub files: Vec<FileInfo>,
    pub total_count: usize,
    /// Display metadata for human-friendly rendering
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub _meta: Option<ToolResultMeta>,
}

pub async fn list_files(args: ListFilesArgs) -> Result<ListFilesResult, ListFilesError> {
    use std::os::unix::fs::PermissionsExt;

    let target_path = args.path.as_deref().unwrap_or(".");
    let include_hidden = args.include_hidden.unwrap_or(false);

    let mut files = Vec::new();

    // Read directory entries
    let entries =
        std::fs::read_dir(target_path).map_err(|e| ListFilesError::ReadDirFailed(e.to_string()))?;

    for entry in entries {
        let entry = entry.map_err(|e| ListFilesError::ReadEntryFailed(e.to_string()))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|e| ListFilesError::MetadataFailed(e.to_string()))?;

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files unless requested
        if !include_hidden && name.starts_with('.') {
            continue;
        }

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
            .and_then(|duration| {
                let secs = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
                chrono::DateTime::from_timestamp(secs, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            });

        files.push(FileInfo {
            name,
            path: path.to_string_lossy().to_string(),
            file_type,
            size: metadata.len(),
            permissions,
            modified,
        });
    }

    // Sort by name for consistent output
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let total_count = files.len();

    let display_meta = ToolDisplayMeta::new(
        "List files",
        format!("{} ({total_count} items)", basename(target_path)),
    );

    Ok(ListFilesResult {
        status: "success".to_string(),
        directory: target_path.to_string(),
        files,
        total_count,
        _meta: Some(display_meta.into()),
    })
}

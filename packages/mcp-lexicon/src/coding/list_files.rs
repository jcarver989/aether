use aether::fs::{FileInfo as FsFileInfo, FileType as FsFileType, Fs};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

impl From<FsFileType> for FileType {
    fn from(fs_type: FsFileType) -> Self {
        match fs_type {
            FsFileType::File => FileType::File,
            FsFileType::Directory => FileType::Directory,
            FsFileType::Symlink => FileType::Symlink,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub file_type: FileType,
    pub size: u64,
    pub permissions: String,
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListFilesResult {
    pub status: String,
    pub directory: String,
    pub files: Vec<FileInfo>,
    pub total_count: usize,
}

pub async fn list_files(
    fs_impl: &impl Fs,
    args: ListFilesArgs,
) -> Result<ListFilesResult, String> {
    let target_path = args.path.as_deref().unwrap_or(".");
    let include_hidden = args.include_hidden.unwrap_or(false);

    // Get file listing from the Fs implementation
    let fs_files = fs_impl
        .list_files_detailed(target_path)
        .await
        .map_err(|e| format!("Failed to read directory: {e}"))?;

    let mut files = Vec::new();

    for fs_file in fs_files {
        // Skip hidden files unless requested
        if !include_hidden && fs_file.name.starts_with('.') {
            continue;
        }

        // Convert timestamp to formatted date if available
        let modified = fs_file.modified.and_then(|timestamp_str| {
            timestamp_str.parse::<i64>().ok().and_then(|secs| {
                chrono::DateTime::from_timestamp(secs, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            })
        });

        files.push(FileInfo {
            name: fs_file.name,
            path: fs_file.path,
            file_type: fs_file.file_type.into(),
            size: fs_file.size,
            permissions: fs_file.permissions,
            modified,
        });
    }

    // Sort by name for consistent output
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let total_count = files.len();

    Ok(ListFilesResult {
        status: "success".to_string(),
        directory: target_path.to_string(),
        files,
        total_count,
    })
}

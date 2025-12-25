use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::coding::error::FileError;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileArgs {
    /// The absolute path to the file to write
    pub file_path: String,
    /// The content to write to the file
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileResponse {
    /// Success message
    pub message: String,
    /// Number of bytes written
    pub bytes_written: usize,
    /// File path that was written
    pub file_path: String,
}

pub async fn write_file_contents(args: WriteFileArgs) -> Result<WriteFileResponse, FileError> {
    let file_path = Path::new(&args.file_path);

    // Create parent directories if needed
    if let Some(parent) = file_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(FileError::CreateDirFailed {
            path: args.file_path,
            reason: e.to_string(),
        });
    }

    // Write content to file
    if let Err(e) = std::fs::write(&args.file_path, &args.content) {
        return Err(FileError::WriteFailed {
            path: args.file_path,
            reason: e.to_string(),
        });
    }

    // Count bytes for response
    let bytes_written = args.content.len();

    Ok(WriteFileResponse {
        message: format!(
            "Successfully wrote {} bytes to {}",
            bytes_written, args.file_path
        ),
        bytes_written,
        file_path: args.file_path,
    })
}

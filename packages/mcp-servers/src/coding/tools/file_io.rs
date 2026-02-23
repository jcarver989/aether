use std::io::ErrorKind;

use crate::coding::error::FileError;

pub async fn read_text_file(path: &str) -> Result<String, FileError> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => FileError::NotFound {
                path: path.to_string(),
            },
            _ => FileError::ReadFailed {
                path: path.to_string(),
                reason: error.to_string(),
            },
        })
}

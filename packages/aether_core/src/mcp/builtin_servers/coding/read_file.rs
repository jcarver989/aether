use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Path to the file to read (must be an existing file)
    pub file_path: String,
}

pub async fn read_file_contents(args: ReadFileArgs) -> Result<serde_json::Value, String> {
    let file_path = Path::new(&args.file_path);

    if !file_path.exists() {
        return Err(format!("File does not exist: {}", args.file_path));
    }

    if !file_path.is_file() {
        return Err(format!("Path is not a file: {}", args.file_path));
    }

    match fs::read_to_string(file_path).await {
        Ok(content) => Ok(serde_json::json!({
            "status": "success",
            "file_path": args.file_path,
            "content": content,
            "size": content.len()
        })),
        Err(e) => Err(format!("Failed to read file {}: {}", args.file_path, e)),
    }
}
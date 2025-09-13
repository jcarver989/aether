use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    pub file_path: String,
    pub content: String,
    pub append: Option<bool>,
}

pub async fn write_file_contents(args: WriteFileArgs) -> Result<serde_json::Value, String> {
    let file_path = Path::new(&args.file_path);
    let append_mode = args.append.unwrap_or(false);

    if let Some(parent) = file_path.parent() {
        if let Err(e) = fs::create_dir_all(parent).await {
            return Err(format!(
                "Failed to create directories for {}: {}",
                args.file_path, e
            ));
        }
    }

    let result = if append_mode {
        match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .await
        {
            Ok(mut file) => {
                use tokio::io::AsyncWriteExt;
                file.write_all(args.content.as_bytes()).await
            }
            Err(e) => Err(e),
        }
    } else {
        fs::write(file_path, &args.content).await
    };

    match result {
        Ok(_) => Ok(serde_json::json!({
            "status": "success",
            "file_path": args.file_path,
            "operation": if append_mode { "appended" } else { "written" },
            "size": args.content.len()
        })),
        Err(e) => Err(format!("Failed to write to file {}: {}", args.file_path, e)),
    }
}
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditFileArgs {
    /// Path to the file to edit
    pub file_path: String,
    /// Exact string to find and replace in the file
    pub old_string: String,
    /// String to replace it with
    pub new_string: String,
    /// Replace all occurrences (default: false - replace only first match)
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditFileResponse {
    pub status: String,
    /// Path of the file that was edited
    pub file_path: String,
    /// Total number of lines in the file after editing
    pub total_lines: usize,
    /// Number of replacements made
    pub replacements_made: usize,
}

pub async fn edit_file_contents(args: EditFileArgs) -> Result<EditFileResponse, String> {
    let file_path = Path::new(&args.file_path);

    // File must exist for editing
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", args.file_path));
    }

    // Read current file content
    let current_content = match fs::read_to_string(file_path).await {
        Ok(content) => content,
        Err(e) => {
            return Err(format!(
                "Failed to read file {}: {}",
                args.file_path, e
            ));
        }
    };

    // Perform string replacement
    let (updated_content, replacements_made) = if args.replace_all {
        let count = current_content.matches(&args.old_string).count();
        (current_content.replace(&args.old_string, &args.new_string), count)
    } else {
        if current_content.contains(&args.old_string) {
            (current_content.replacen(&args.old_string, &args.new_string, 1), 1)
        } else {
            (current_content.clone(), 0)
        }
    };

    // Check if any replacement actually occurred
    if replacements_made == 0 {
        return Err(format!(
            "String replacement failed for file {}: string '{}' not found",
            args.file_path, args.old_string
        ));
    }

    // Write back to file
    if let Err(e) = fs::write(file_path, &updated_content).await {
        return Err(format!("Failed to write to file {}: {}", args.file_path, e));
    }

    // Count lines for response
    let total_lines = updated_content.lines().count();

    Ok(EditFileResponse {
        status: "success".to_string(),
        file_path: args.file_path,
        total_lines,
        replacements_made,
    })
}

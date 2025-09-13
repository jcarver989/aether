use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FindArgs {
    /// Filename pattern to match (supports wildcards like *.rs, main.*, test*.py)
    pub pattern: String,
    /// Directory path to search recursively (defaults to current directory if not specified)
    pub path: Option<String>,
    /// Whether to perform case-insensitive filename matching (defaults to false)
    pub case_insensitive: Option<bool>,
}

pub async fn find_files_by_name(args: FindArgs) -> Result<serde_json::Value, String> {
    let search_path = args.path.as_deref().unwrap_or(".");
    let mut matching_files = Vec::new();
    let case_insensitive = args.case_insensitive.unwrap_or(false);

    let walker = WalkBuilder::new(search_path)
        .hidden(false)
        .git_ignore(true)
        .build();

    for result in walker {
        match result {
            Ok(entry) => {
                if entry.file_type().map_or(false, |ft| ft.is_file()) {
                    if let Some(filename) = entry.path().file_name() {
                        let filename_str = filename.to_string_lossy();
                        let pattern = &args.pattern;

                        let matches = if case_insensitive {
                            pattern_matches(
                                &filename_str.to_lowercase(),
                                &pattern.to_lowercase(),
                            )
                        } else {
                            pattern_matches(&filename_str, pattern)
                        };

                        if matches {
                            matching_files.push(entry.path().to_string_lossy().to_string());
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }

    Ok(serde_json::json!({
        "status": "success",
        "pattern": args.pattern,
        "path": search_path,
        "files": matching_files,
        "file_count": matching_files.len()
    }))
}

fn pattern_matches(filename: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        let regex_pattern = pattern.replace(".", "\\.").replace("*", ".*");
        if let Ok(regex) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
            return regex.is_match(filename);
        }
    }
    filename == pattern || filename.contains(pattern)
}
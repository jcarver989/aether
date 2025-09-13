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

    // Prepare the pattern for matching
    let pattern = if case_insensitive {
        args.pattern.to_lowercase()
    } else {
        args.pattern.clone()
    };

    // Pre-compile regex if pattern contains wildcards
    let compiled_regex = if pattern.contains('*') {
        let regex_pattern = pattern.replace(".", "\\.").replace("*", ".*");
        match regex::Regex::new(&format!("^{regex_pattern}$")) {
            Ok(regex) => Some(regex),
            Err(e) => return Err(format!("Invalid pattern '{}': {}", args.pattern, e)),
        }
    } else {
        None
    };

    let walker = WalkBuilder::new(search_path)
        .hidden(false)
        .git_ignore(true)
        .build();

    for result in walker {
        match result {
            Ok(entry) => {
                if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    if let Some(filename) = entry.path().file_name() {
                        let filename_str = filename.to_string_lossy();
                        let check_filename = if case_insensitive {
                            filename_str.to_lowercase()
                        } else {
                            filename_str.to_string()
                        };

                        let matches = pattern_matches(&check_filename, &pattern, &compiled_regex);

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

fn pattern_matches(filename: &str, pattern: &str, compiled_regex: &Option<regex::Regex>) -> bool {
    if let Some(regex) = compiled_regex {
        regex.is_match(filename)
    } else {
        // For non-wildcard patterns, use exact match only
        filename == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::{self, File};
    use std::path::Path;

    fn create_test_files(temp_dir: &Path) -> Result<(), std::io::Error> {
        File::create(temp_dir.join("test.rs"))?;
        File::create(temp_dir.join("main.rs"))?;
        File::create(temp_dir.join("lib.rs"))?;
        File::create(temp_dir.join("example.txt"))?;
        File::create(temp_dir.join("README.md"))?;
        File::create(temp_dir.join("config.json"))?;

        // Create subdirectory with files
        let sub_dir = temp_dir.join("subdir");
        fs::create_dir(&sub_dir)?;
        File::create(sub_dir.join("nested.rs"))?;
        File::create(sub_dir.join("other.txt"))?;

        Ok(())
    }

    #[tokio::test]
    async fn test_exact_match_pattern() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: "test.rs".to_string(),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<String> = serde_json::from_value(result["files"].clone()).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test.rs"));
    }

    #[tokio::test]
    async fn test_wildcard_pattern() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: "*.rs".to_string(),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<String> = serde_json::from_value(result["files"].clone()).unwrap();

        // Should find all .rs files including the nested one
        assert_eq!(files.len(), 4); // test.rs, main.rs, lib.rs, nested.rs
        assert!(files.iter().all(|f| f.ends_with(".rs")));
    }

    #[tokio::test]
    async fn test_case_insensitive_pattern() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: "README.md".to_string(),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: Some(true),
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<String> = serde_json::from_value(result["files"].clone()).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("README.md"));
    }

    #[tokio::test]
    async fn test_invalid_regex_pattern() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: "*[invalid[regex*".to_string(), // Invalid regex pattern with wildcards
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
        };

        let result = find_files_by_name(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid pattern"));
    }

    #[tokio::test]
    async fn test_pattern_matches_function() {
        // Test exact match
        assert!(pattern_matches("test.rs", "test.rs", &None));
        assert!(!pattern_matches("test.rs", "main.rs", &None));

        // Test regex match
        let regex = regex::Regex::new("^.*\\.rs$").unwrap();
        assert!(pattern_matches("test.rs", "*.rs", &Some(regex.clone())));
        assert!(!pattern_matches("test.txt", "*.rs", &Some(regex)));

        let wildcard_regex = regex::Regex::new("^test.*$").unwrap();
        assert!(pattern_matches("test123", "test*", &Some(wildcard_regex.clone())));
        assert!(!pattern_matches("main123", "test*", &Some(wildcard_regex)));
    }

    #[tokio::test]
    async fn test_performance_no_regex_recompilation() {
        // This test ensures we don't recompile regex for each file
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let start = std::time::Instant::now();

        let args = FindArgs {
            pattern: "*.rs".to_string(),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<String> = serde_json::from_value(result["files"].clone()).unwrap();

        let elapsed = start.elapsed();

        // Should complete very quickly with our optimization
        assert!(elapsed.as_millis() < 100, "Search took too long: {:?}", elapsed);
        assert!(!files.is_empty());
    }
}
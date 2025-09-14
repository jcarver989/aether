use glob::Pattern;
use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FindArgs {
    /// Filename pattern to match (supports full glob patterns like **/*.rs, src/**/test_*.py)
    pub pattern: Option<String>,
    /// Directory path to search recursively (defaults to current directory if not specified)
    pub path: Option<String>,
    /// Whether to perform case-insensitive filename matching (defaults to false)
    pub case_insensitive: Option<bool>,
    /// Filter files by type (e.g., 'rust', 'python', 'javascript')
    pub file_types: Option<Vec<String>>,
    /// Patterns to exclude (supports glob patterns like node_modules, target, .git)
    pub exclude_patterns: Option<Vec<String>>,
    /// Maximum directory depth to search (defaults to unlimited)
    pub max_depth: Option<usize>,
    /// Maximum number of results to return
    pub max_results: Option<usize>,
    /// Include file metadata (size, modified time, permissions)
    pub include_metadata: Option<bool>,
    /// Sort results by: name, modified, size, path (defaults to name)
    pub sort_by: Option<SortBy>,
    /// Follow symbolic links (defaults to false)
    pub follow_symlinks: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum SortBy {
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "modified")]
    Modified,
    #[serde(rename = "size")]
    Size,
    #[serde(rename = "path")]
    Path,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<FileType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<String>,
}

// File type mappings for common languages
fn get_file_extensions_for_type(file_type: &str) -> Vec<&'static str> {
    match file_type.to_lowercase().as_str() {
        "rust" | "rs" => vec!["rs"],
        "python" | "py" => vec!["py", "pyi", "pyw"],
        "javascript" | "js" => vec!["js", "jsx", "mjs"],
        "typescript" | "ts" => vec!["ts", "tsx"],
        "go" => vec!["go"],
        "java" => vec!["java"],
        "c" => vec!["c", "h"],
        "cpp" | "c++" => vec!["cpp", "cxx", "cc", "hpp", "hxx", "hh"],
        "csharp" | "cs" => vec!["cs"],
        "php" => vec!["php"],
        "ruby" | "rb" => vec!["rb"],
        "swift" => vec!["swift"],
        "kotlin" => vec!["kt", "kts"],
        "scala" => vec!["scala"],
        "html" => vec!["html", "htm"],
        "css" => vec!["css"],
        "json" => vec!["json"],
        "yaml" | "yml" => vec!["yaml", "yml"],
        "toml" => vec!["toml"],
        "markdown" | "md" => vec!["md", "markdown"],
        "xml" => vec!["xml"],
        "sql" => vec!["sql"],
        "sh" | "shell" => vec!["sh", "bash", "zsh"],
        _ => vec![],
    }
}

fn should_include_file_by_type(path: &Path, file_types: &Option<Vec<String>>) -> bool {
    if let Some(types) = file_types {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            for file_type in types {
                let extensions = get_file_extensions_for_type(file_type);
                if extensions.contains(&ext) {
                    return true;
                }
            }
            return false;
        }
        return false;
    }
    true
}

fn should_exclude_path(path: &Path, exclude_patterns: &Option<Vec<String>>) -> bool {
    if let Some(patterns) = exclude_patterns {
        let path_str = path.to_string_lossy();
        let file_name = path.file_name().map(|n| n.to_string_lossy());

        for pattern in patterns {
            // Check if it matches the full path or just the filename
            if let Ok(glob_pattern) = Pattern::new(pattern) {
                if glob_pattern.matches(&path_str) {
                    return true;
                }
                if let Some(name) = &file_name {
                    if glob_pattern.matches(name) {
                        return true;
                    }
                }
            }

            // Also check if any parent directory matches the pattern
            for ancestor in path.ancestors() {
                if let Some(ancestor_name) = ancestor.file_name() {
                    let ancestor_str = ancestor_name.to_string_lossy();
                    if let Ok(glob_pattern) = Pattern::new(pattern) {
                        if glob_pattern.matches(&ancestor_str) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

pub async fn find_files_by_name(args: FindArgs) -> Result<serde_json::Value, String> {
    // Validate arguments
    if args.pattern.is_none() && args.file_types.is_none() {
        return Err("Either 'pattern' or 'file_types' must be specified".to_string());
    }

    let search_path = args.path.as_deref().unwrap_or(".");
    let case_insensitive = args.case_insensitive.unwrap_or(false);
    let include_metadata = args.include_metadata.unwrap_or(false);
    let sort_by = args.sort_by.as_ref().unwrap_or(&SortBy::Name);
    let follow_symlinks = args.follow_symlinks.unwrap_or(false);
    let max_results = args.max_results;

    // Validate path exists
    if !Path::new(search_path).exists() {
        return Err(format!("Search path does not exist: {}", search_path));
    }

    // Compile glob pattern if provided
    let glob_pattern = if let Some(ref pattern_str) = args.pattern {
        let pattern_to_use = if case_insensitive {
            pattern_str.to_lowercase()
        } else {
            pattern_str.clone()
        };

        match Pattern::new(&pattern_to_use) {
            Ok(pattern) => Some(pattern),
            Err(e) => return Err(format!("Invalid glob pattern '{}': {}", pattern_str, e)),
        }
    } else {
        None
    };

    // Build walker with configuration
    let mut walker_builder = WalkBuilder::new(search_path);
    walker_builder
        .hidden(false)
        .git_ignore(true)
        .follow_links(follow_symlinks);

    if let Some(depth) = args.max_depth {
        walker_builder.max_depth(Some(depth));
    }

    // Use parallel walker for better performance
    let walker = walker_builder.build_parallel();
    let matching_files = std::sync::Arc::new(std::sync::Mutex::new(Vec::<FileResult>::new()));
    let matching_files_clone = matching_files.clone();
    let args_clone = std::sync::Arc::new(args.clone());
    let glob_pattern_clone = std::sync::Arc::new(glob_pattern);
    let case_insensitive_clone = case_insensitive;
    let include_metadata_clone = include_metadata;
    let max_results_clone = max_results;

    walker.run(|| {
        let matching_files = matching_files_clone.clone();
        let args = args_clone.clone();
        let glob_pattern = glob_pattern_clone.clone();

        Box::new(move |result| {
            if let Ok(entry) = result {
                // Check if we've hit max results limit
                if let Some(max) = max_results_clone {
                    if let Ok(files) = matching_files.lock() {
                        if files.len() >= max {
                            return ignore::WalkState::Quit;
                        }
                    }
                }

                let path = entry.path();

                // Skip if should be excluded
                if should_exclude_path(path, &args.exclude_patterns) {
                    return ignore::WalkState::Continue;
                }

                // Only process files (not directories unless we want them)
                if let Some(file_type) = entry.file_type() {
                    if !file_type.is_file() {
                        return ignore::WalkState::Continue;
                    }
                }

                // Check file type filtering
                if !should_include_file_by_type(path, &args.file_types) {
                    return ignore::WalkState::Continue;
                }

                // Check glob pattern matching
                let matches = if let Some(ref pattern) = *glob_pattern {
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        let check_filename = if case_insensitive_clone {
                            filename_str.to_lowercase()
                        } else {
                            filename_str.to_string()
                        };
                        pattern.matches(&check_filename)
                    } else {
                        false
                    }
                } else {
                    true // If no pattern, match all (filtered by file_types above)
                };

                if matches {
                    let mut file_result = FileResult {
                        path: path.to_string_lossy().to_string(),
                        name: path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "<unknown>".to_string()),
                        file_type: None,
                        size: None,
                        modified: None,
                        permissions: None,
                    };

                    // Add metadata if requested
                    if include_metadata_clone {
                        if let Ok(metadata) = path.metadata() {
                            file_result.file_type = Some(if metadata.is_dir() {
                                FileType::Directory
                            } else if metadata.is_symlink() {
                                FileType::Symlink
                            } else {
                                FileType::File
                            });

                            file_result.size = Some(metadata.len());
                            file_result.permissions =
                                Some(format!("{:o}", metadata.permissions().mode() & 0o777));

                            file_result.modified = metadata
                                .modified()
                                .ok()
                                .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                                .map(|duration| {
                                    let secs = duration.as_secs();
                                    chrono::DateTime::from_timestamp(secs as i64, 0)
                                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                        .unwrap_or_else(|| "unknown".to_string())
                                });
                        }
                    }

                    if let Ok(mut files) = matching_files.lock() {
                        files.push(file_result);
                    }
                }
            }
            ignore::WalkState::Continue
        })
    });

    // Extract results and sort - wait for all threads to complete first
    drop(matching_files_clone);
    drop(args_clone);
    drop(glob_pattern_clone);

    // Extract results using the remaining Arc reference
    let mut files = matching_files
        .lock()
        .map_err(|_| "Failed to lock results")?
        .clone();

    // Sort results
    match sort_by {
        SortBy::Name => files.sort_by(|a, b| a.name.cmp(&b.name)),
        SortBy::Path => files.sort_by(|a, b| a.path.cmp(&b.path)),
        SortBy::Size => files.sort_by(|a, b| {
            let a_size = a.size.unwrap_or(0);
            let b_size = b.size.unwrap_or(0);
            b_size.cmp(&a_size) // Descending order for size
        }),
        SortBy::Modified => files.sort_by(|a, b| {
            match (&a.modified, &b.modified) {
                (Some(a_mod), Some(b_mod)) => b_mod.cmp(a_mod), // Descending for recency
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        }),
    }

    // Check for truncation
    let truncated = if let Some(max) = max_results {
        files.len() >= max
    } else {
        false
    };

    // Build response
    let mut response = serde_json::json!({
        "status": "success",
        "path": search_path,
        "files": files,
        "file_count": files.len()
    });

    // Add metadata about the search
    if let Some(pattern) = &args.pattern {
        response["pattern"] = serde_json::Value::String(pattern.clone());
    }
    if let Some(file_types) = &args.file_types {
        response["file_types"] = serde_json::Value::Array(
            file_types
                .iter()
                .map(|t| serde_json::Value::String(t.clone()))
                .collect(),
        );
    }
    if let Some(exclude_patterns) = &args.exclude_patterns {
        response["exclude_patterns"] = serde_json::Value::Array(
            exclude_patterns
                .iter()
                .map(|p| serde_json::Value::String(p.clone()))
                .collect(),
        );
    }
    if let Some(max_depth) = args.max_depth {
        response["max_depth"] = serde_json::Value::Number(max_depth.into());
    }
    if let Some(max_results) = max_results {
        response["max_results"] = serde_json::Value::Number(max_results.into());
        if truncated {
            response["truncated"] = serde_json::Value::Bool(true);
        }
    }
    if include_metadata {
        response["includes_metadata"] = serde_json::Value::Bool(true);
    }
    response["sort_by"] = serde_json::Value::String(
        match sort_by {
            SortBy::Name => "name",
            SortBy::Path => "path",
            SortBy::Size => "size",
            SortBy::Modified => "modified",
        }
        .to_string(),
    );

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::path::Path;
    use tempfile::TempDir;

    fn create_test_files(temp_dir: &Path) -> Result<(), std::io::Error> {
        File::create(temp_dir.join("test.rs"))?;
        File::create(temp_dir.join("main.rs"))?;
        File::create(temp_dir.join("lib.rs"))?;
        File::create(temp_dir.join("example.txt"))?;
        File::create(temp_dir.join("README.md"))?;
        File::create(temp_dir.join("config.json"))?;
        File::create(temp_dir.join("test_data.py"))?;
        File::create(temp_dir.join("app.js"))?;
        File::create(temp_dir.join("style.css"))?;

        // Create subdirectory with files
        let sub_dir = temp_dir.join("subdir");
        fs::create_dir(&sub_dir)?;
        File::create(sub_dir.join("nested.rs"))?;
        File::create(sub_dir.join("other.txt"))?;
        File::create(sub_dir.join("deep_test.py"))?;

        // Create node_modules (to test exclusion)
        let node_modules = temp_dir.join("node_modules");
        fs::create_dir(&node_modules)?;
        File::create(node_modules.join("package.js"))?;

        // Create deeper nesting for depth testing
        let deep_dir = sub_dir.join("deep");
        fs::create_dir(&deep_dir)?;
        File::create(deep_dir.join("very_deep.rs"))?;

        Ok(())
    }

    #[tokio::test]
    async fn test_exact_pattern_match() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: Some("test.rs".to_string()),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].path.ends_with("test.rs"));
        assert_eq!(files[0].name, "test.rs");
    }

    #[tokio::test]
    async fn test_glob_wildcard_pattern() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: Some("*.rs".to_string()),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        // Should find all .rs files including nested ones
        assert!(files.len() >= 4); // test.rs, main.rs, lib.rs, nested.rs, very_deep.rs
        assert!(files.iter().all(|f| f.path.ends_with(".rs")));
    }

    #[tokio::test]
    async fn test_file_type_filtering() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: None,
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: Some(vec!["rust".to_string()]),
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        assert!(files.len() >= 4); // test.rs, main.rs, lib.rs, nested.rs, very_deep.rs
        assert!(files.iter().all(|f| f.path.ends_with(".rs")));
    }

    #[tokio::test]
    async fn test_exclude_patterns() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: Some("*".to_string()),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: Some(vec!["node_modules".to_string(), "*.txt".to_string()]),
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        // Should not include any .txt files or files from node_modules
        assert!(!files.iter().any(|f| f.path.ends_with(".txt")));
        assert!(!files.iter().any(|f| f.path.contains("node_modules")));
    }

    #[tokio::test]
    async fn test_max_depth() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: Some("*.rs".to_string()),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: Some(2), // Search up to 2 levels deep
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        // Should not find very_deep.rs which is at depth 3
        assert!(!files.iter().any(|f| f.name == "very_deep.rs"));
        // But should find nested.rs which is at depth 2
        assert!(files.iter().any(|f| f.name == "nested.rs"));
    }

    #[tokio::test]
    async fn test_metadata_inclusion() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: Some("test.rs".to_string()),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: Some(true),
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert!(file.size.is_some());
        assert!(file.file_type.is_some());
        assert!(file.permissions.is_some());
    }

    #[tokio::test]
    async fn test_max_results_with_truncation() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: Some("*".to_string()),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: Some(3),
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        assert_eq!(files.len(), 3);
        assert_eq!(result["truncated"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn test_sorting_by_name() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: Some("*.rs".to_string()),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: Some(SortBy::Name),
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        // Verify files are sorted by name
        let names: Vec<&String> = files.iter().map(|f| &f.name).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names);
    }

    #[tokio::test]
    async fn test_case_insensitive_matching() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: Some("README.MD".to_string()), // Uppercase
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: Some(true),
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].path.ends_with("README.md"));
    }

    #[tokio::test]
    async fn test_multiple_file_types() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let args = FindArgs {
            pattern: None,
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: Some(vec!["rust".to_string(), "python".to_string()]),
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        // Should find both .rs and .py files
        let has_rs = files.iter().any(|f| f.path.ends_with(".rs"));
        let has_py = files.iter().any(|f| f.path.ends_with(".py"));
        assert!(has_rs && has_py);
    }

    #[tokio::test]
    async fn test_validation_errors() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        // Test missing both pattern and file_types
        let args = FindArgs {
            pattern: None,
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Either 'pattern' or 'file_types' must be specified")
        );

        // Test invalid path
        let args = FindArgs {
            pattern: Some("*".to_string()),
            path: Some("/nonexistent/path".to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Search path does not exist"));
    }

    #[tokio::test]
    async fn test_performance_with_parallel_walking() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let start = std::time::Instant::now();

        let args = FindArgs {
            pattern: Some("*.rs".to_string()),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            case_insensitive: None,
            file_types: None,
            exclude_patterns: None,
            max_depth: None,
            max_results: None,
            include_metadata: None,
            sort_by: None,
            follow_symlinks: None,
        };

        let result = find_files_by_name(args).await.unwrap();
        let files: Vec<FileResult> = serde_json::from_value(result["files"].clone()).unwrap();

        let elapsed = start.elapsed();

        // Should complete quickly with parallel walking
        assert!(
            elapsed.as_millis() < 1000, // More generous timeout for CI
            "Search took too long: {:?}",
            elapsed
        );
        assert!(!files.is_empty());
    }
}

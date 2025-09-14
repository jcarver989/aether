use grep::{
    regex::RegexMatcherBuilder,
    searcher::{BinaryDetection, SearcherBuilder},
};
use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::common::{HasMatchSink, MatchCollectorSink, OutputMode};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepArgs {
    /// The regex pattern to search for in file contents
    pub pattern: String,
    /// Directory path to search recursively (defaults to current directory if not specified)
    pub path: Option<String>,
    /// Specific file path to search (overrides path if provided)
    pub file_path: Option<String>,
    /// Output format: 'matches' returns matching lines with context, 'files_only' returns just file paths
    pub output_mode: Option<OutputMode>,
    /// Whether to perform case-insensitive matching (defaults to false)
    pub case_insensitive: Option<bool>,
    /// Whether to include line numbers in output (defaults to true)
    pub line_numbers: Option<bool>,
    /// Number of context lines to show after matches (-A flag)
    pub context_after: Option<u32>,
    /// Number of context lines to show before matches (-B flag)
    pub context_before: Option<u32>,
    /// Number of context lines to show around matches (-C flag, overrides context_after/before)
    pub context_around: Option<u32>,
    /// Filter files by type (e.g., 'rust', 'python', 'javascript')
    pub file_types: Option<Vec<String>>,
    /// Maximum number of results to return
    pub max_results: Option<usize>,
    /// Invert match - show lines that do NOT match the pattern
    pub invert_match: Option<bool>,
    /// Match only whole words (word boundary matching)
    pub word_boundary: Option<bool>,
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

fn should_include_file(path: &Path, file_types: &Option<Vec<String>>) -> bool {
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

pub async fn perform_grep(args: GrepArgs) -> Result<serde_json::Value, String> {
    // Build the regex pattern with optional word boundary
    let pattern = if args.word_boundary.unwrap_or(false) {
        format!(r"\b{}\b", regex::escape(&args.pattern))
    } else {
        args.pattern.clone()
    };

    // Create the matcher with case sensitivity
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(args.case_insensitive.unwrap_or(false))
        .build(&pattern)
        .map_err(|e| format!("Invalid regex pattern: {}", e))?;
    let output_mode = args.output_mode.unwrap_or(OutputMode::Matches);

    // Determine context lines
    let (context_before, context_after) = if let Some(around) = args.context_around {
        (around, around)
    } else {
        (
            args.context_before.unwrap_or(0),
            args.context_after.unwrap_or(0),
        )
    };

    let mut searcher_builder = SearcherBuilder::new();
    searcher_builder
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(args.line_numbers.unwrap_or(true))
        .before_context(context_before as usize)
        .after_context(context_after as usize);

    if args.invert_match.unwrap_or(false) {
        searcher_builder.invert_match(true);
    }

    let mut searcher = searcher_builder.build();

    let mut all_matches = Vec::new();
    let mut files_with_matches = Vec::new();

    // Handle single file search
    if let Some(file_path) = &args.file_path {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(format!("File does not exist: {}", file_path));
        }

        // Check file type filtering
        if !should_include_file(path, &args.file_types) {
            return Ok(serde_json::json!({
                "status": "success",
                "pattern": args.pattern,
                "path": file_path,
                "matches": [],
                "match_count": 0,
                "message": "File type not included in filter"
            }));
        }

        match output_mode {
            OutputMode::Matches => {
                let mut sink = MatchCollectorSink::with_max_results(
                    path,
                    args.line_numbers.unwrap_or(true),
                    args.max_results
                );
                searcher
                    .search_path(&matcher, path, &mut sink)
                    .map_err(|e| format!("Search error: {}", e))?;
                all_matches = sink.matches;
            }
            OutputMode::FilesOnly => {
                let mut sink = HasMatchSink::new();
                searcher
                    .search_path(&matcher, path, &mut sink)
                    .map_err(|e| format!("Search error: {}", e))?;
                if sink.has_match {
                    files_with_matches.push(file_path.clone());
                }
            }
        }
    } else {
        // Directory search
        let search_path = args.path.as_deref().unwrap_or(".");
        let walker = WalkBuilder::new(search_path)
            .hidden(false)
            .git_ignore(true)
            .build();

        let mut total_matches = 0;
        let max_results = args.max_results.unwrap_or(usize::MAX);

        for result in walker {
            // Check if we've reached the global max results limit
            if total_matches >= max_results {
                break;
            }

            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        // Check file type filtering
                        if !should_include_file(entry.path(), &args.file_types) {
                            continue;
                        }

                        match output_mode {
                            OutputMode::Matches => {
                                let remaining_results = max_results.saturating_sub(total_matches);
                                let mut sink = MatchCollectorSink::with_max_results(
                                    entry.path(),
                                    args.line_numbers.unwrap_or(true),
                                    Some(remaining_results)
                                );
                                if let Ok(_) =
                                    searcher.search_path(&matcher, entry.path(), &mut sink)
                                {
                                    total_matches += sink.matches.len();
                                    all_matches.extend(sink.matches);
                                }
                            }
                            OutputMode::FilesOnly => {
                                let mut sink = HasMatchSink::new();
                                if let Ok(_) =
                                    searcher.search_path(&matcher, entry.path(), &mut sink)
                                {
                                    if sink.has_match {
                                        files_with_matches
                                            .push(entry.path().to_string_lossy().to_string());
                                        total_matches += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }
    }

    match output_mode {
        OutputMode::Matches => {
            let search_location = args
                .file_path
                .as_deref()
                .or(args.path.as_deref())
                .unwrap_or(".");
            let mut response = serde_json::json!({
                "status": "success",
                "pattern": args.pattern,
                "path": search_location,
                "matches": all_matches,
                "match_count": all_matches.len()
            });

            // Add metadata about search configuration
            if let Some(max_results) = args.max_results {
                response["max_results"] = serde_json::Value::Number(max_results.into());
                if all_matches.len() >= max_results {
                    response["truncated"] = serde_json::Value::Bool(true);
                }
            }
            if let Some(file_types) = &args.file_types {
                response["file_types"] = serde_json::Value::Array(
                    file_types.iter().map(|t| serde_json::Value::String(t.clone())).collect()
                );
            }
            if args.invert_match.unwrap_or(false) {
                response["invert_match"] = serde_json::Value::Bool(true);
            }
            if args.word_boundary.unwrap_or(false) {
                response["word_boundary"] = serde_json::Value::Bool(true);
            }
            if args.context_around.is_some() || args.context_before.is_some() || args.context_after.is_some() {
                response["context_lines"] = serde_json::json!({
                    "before": args.context_before.unwrap_or(args.context_around.unwrap_or(0)),
                    "after": args.context_after.unwrap_or(args.context_around.unwrap_or(0))
                });
            }

            Ok(response)
        }
        OutputMode::FilesOnly => {
            let search_location = args
                .file_path
                .as_deref()
                .or(args.path.as_deref())
                .unwrap_or(".");
            let mut response = serde_json::json!({
                "status": "success",
                "pattern": args.pattern,
                "path": search_location,
                "files": files_with_matches,
                "file_count": files_with_matches.len()
            });

            // Add metadata about search configuration
            if let Some(max_results) = args.max_results {
                response["max_results"] = serde_json::Value::Number(max_results.into());
                if files_with_matches.len() >= max_results {
                    response["truncated"] = serde_json::Value::Bool(true);
                }
            }
            if let Some(file_types) = &args.file_types {
                response["file_types"] = serde_json::Value::Array(
                    file_types.iter().map(|t| serde_json::Value::String(t.clone())).collect()
                );
            }
            if args.invert_match.unwrap_or(false) {
                response["invert_match"] = serde_json::Value::Bool(true);
            }

            Ok(response)
        }
    }
}

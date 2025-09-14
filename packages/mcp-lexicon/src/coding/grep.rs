use grep::{
    regex::RegexMatcher,
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
    /// Number of context lines to show around matches (not currently implemented)
    pub context: Option<u32>,
}

pub async fn perform_grep(args: GrepArgs) -> Result<serde_json::Value, String> {
    let matcher = RegexMatcher::new_line_matcher(&args.pattern)
        .map_err(|e| format!("Invalid regex pattern: {}", e))?;
    let output_mode = args.output_mode.unwrap_or(OutputMode::Matches);

    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(args.line_numbers.unwrap_or(true))
        .build();

    let mut all_matches = Vec::new();
    let mut files_with_matches = Vec::new();

    // Handle single file search
    if let Some(file_path) = &args.file_path {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(format!("File does not exist: {}", file_path));
        }

        match output_mode {
            OutputMode::Matches => {
                let mut sink = MatchCollectorSink::new(path, args.line_numbers.unwrap_or(true));
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

        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        match output_mode {
                            OutputMode::Matches => {
                                let mut sink = MatchCollectorSink::new(
                                    entry.path(),
                                    args.line_numbers.unwrap_or(true),
                                );
                                if let Ok(_) =
                                    searcher.search_path(&matcher, entry.path(), &mut sink)
                                {
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
            Ok(serde_json::json!({
                "status": "success",
                "pattern": args.pattern,
                "path": search_location,
                "matches": all_matches,
                "match_count": all_matches.len()
            }))
        }
        OutputMode::FilesOnly => {
            let search_location = args
                .file_path
                .as_deref()
                .or(args.path.as_deref())
                .unwrap_or(".");
            Ok(serde_json::json!({
                "status": "success",
                "pattern": args.pattern,
                "path": search_location,
                "files": files_with_matches,
                "file_count": files_with_matches.len()
            }))
        }
    }
}

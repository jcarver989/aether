pub mod common;

use crate::display_meta::ToolDisplayMeta;
use crate::error::GrepError;
use aether_lspd::extensions_for_alias as extensions_for_type;
use common::{CountSink, HasMatchSink, MatchCollectorSink, MatchData, OutputMode};
use globset::{Glob, GlobSetBuilder};
use grep::{
    regex::RegexMatcherBuilder,
    searcher::{BinaryDetection, SearcherBuilder},
};
use ignore::{WalkBuilder, WalkState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrepContentOutput {
    /// Matching lines with context
    pub matches: Vec<MatchData>,
    /// Total number of matches
    pub total_matches: usize,
    /// Display metadata for human-friendly rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrepFilesOutput {
    /// Files containing matches
    pub files: Vec<String>,
    /// Number of files with matches
    pub count: usize,
    /// Display metadata for human-friendly rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrepFileCount {
    pub file: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrepCountOutput {
    /// Match counts per file
    pub counts: Vec<GrepFileCount>,
    /// Total matches across all files
    pub total: usize,
    /// Display metadata for human-friendly rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[schemars(extend("type" = "object"))]
pub enum GrepOutput {
    Content(GrepContentOutput),
    Files(GrepFilesOutput),
    Count(GrepCountOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrepInput {
    /// The regular expression pattern to search for
    pub pattern: String,
    /// File or directory to search in (defaults to cwd)
    pub path: Option<String>,
    /// Glob pattern to filter files (e.g. "*.js")
    pub glob: Option<String>,
    /// File type to search (e.g. "js", "py", "rust")
    #[serde(rename = "type")]
    pub file_type: Option<String>,
    /// Output mode: "content", "`files_with_matches`", or "count"
    pub output_mode: Option<OutputMode>,
    /// Case insensitive search
    #[serde(rename = "-i")]
    pub case_insensitive: Option<bool>,
    /// Show line numbers (for content mode)
    #[serde(rename = "-n")]
    pub line_numbers: Option<bool>,
    /// Lines to show before each match
    #[serde(rename = "-B")]
    pub context_before: Option<u32>,
    /// Lines to show after each match
    #[serde(rename = "-A")]
    pub context_after: Option<u32>,
    /// Lines to show before and after each match
    #[serde(rename = "-C")]
    pub context_around: Option<u32>,
    /// Limit output to first N lines/entries
    pub head_limit: Option<usize>,
    /// Enable multiline mode
    pub multiline: Option<bool>,
}

/// Thread-safe state for parallel grep execution
struct ParallelGrepState {
    matches: Mutex<Vec<MatchData>>,
    files_with_matches: Mutex<Vec<String>>,
    file_counts: Mutex<Vec<GrepFileCount>>,
    total_items: AtomicUsize,
    max_items: usize,
    limit_reached: AtomicBool,
}

impl ParallelGrepState {
    fn new(max_items: usize) -> Self {
        Self {
            matches: Mutex::new(Vec::new()),
            files_with_matches: Mutex::new(Vec::new()),
            file_counts: Mutex::new(Vec::new()),
            total_items: AtomicUsize::new(0),
            max_items,
            limit_reached: AtomicBool::new(false),
        }
    }
}

fn should_include_file(
    path: &Path,
    file_type: &Option<String>,
    glob_pattern: &Option<globset::GlobSet>,
) -> bool {
    // Check glob pattern first (more specific)
    if let Some(glob_set) = glob_pattern {
        return glob_set.is_match(path);
    }

    // Check file type filter
    if let Some(ftype) = file_type {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let extensions = extensions_for_type(ftype);
            return extensions.contains(&ext);
        }
        return false;
    }

    true
}

pub async fn perform_grep(args: GrepInput) -> Result<GrepOutput, GrepError> {
    // Build glob set if glob pattern is provided
    let glob_set = if let Some(glob_pattern) = &args.glob {
        let mut builder = GlobSetBuilder::new();
        builder.add(
            Glob::new(glob_pattern).map_err(|e| GrepError::InvalidGlobPattern {
                pattern: glob_pattern.clone(),
                reason: e.to_string(),
            })?,
        );
        Some(
            builder
                .build()
                .map_err(|e| GrepError::GlobSetBuildFailed(e.to_string()))?,
        )
    } else {
        None
    };

    // Create the matcher with case sensitivity and multiline support
    let mut matcher_builder = RegexMatcherBuilder::new();
    matcher_builder.case_insensitive(args.case_insensitive.unwrap_or(false));

    if args.multiline.unwrap_or(false) {
        matcher_builder.multi_line(true).dot_matches_new_line(true);
    }

    let matcher = matcher_builder
        .build(&args.pattern)
        .map_err(|e| GrepError::InvalidRegex(e.to_string()))?;

    let output_mode = args.output_mode.unwrap_or(OutputMode::Content);

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

    // Enable multiline in searcher if multiline pattern matching is requested
    if args.multiline.unwrap_or(false) {
        searcher_builder.multi_line(true);
    }

    let mut searcher = searcher_builder.build();

    let mut all_matches = Vec::new();
    let mut files_with_matches = Vec::new();
    let mut file_counts = Vec::new();

    // Directory search
    let search_path = args.path.as_deref().unwrap_or(".");
    let path_obj = Path::new(search_path);

    // Determine if searching a single file or directory
    let is_single_file = path_obj.is_file();

    if is_single_file {
        // Single file search
        if !should_include_file(path_obj, &args.file_type, &glob_set) {
            // Return empty results if file doesn't match filters
            let empty_meta =
                ToolDisplayMeta::grep(args.pattern.clone(), search_path.to_string(), 0);
            let meta = empty_meta.into_meta();
            return match output_mode {
                OutputMode::Content => Ok(GrepOutput::Content(GrepContentOutput {
                    matches: vec![],
                    total_matches: 0,
                    _meta: meta,
                })),
                OutputMode::FilesWithMatches => Ok(GrepOutput::Files(GrepFilesOutput {
                    files: vec![],
                    count: 0,
                    _meta: meta,
                })),
                OutputMode::Count => Ok(GrepOutput::Count(GrepCountOutput {
                    counts: vec![],
                    total: 0,
                    _meta: meta,
                })),
            };
        }

        // Search the single file
        match output_mode {
            OutputMode::Content => {
                let mut sink = MatchCollectorSink::with_max_results(
                    path_obj,
                    args.line_numbers.unwrap_or(true),
                    args.head_limit,
                );
                searcher
                    .search_path(&matcher, path_obj, &mut sink)
                    .map_err(|e| GrepError::SearchFailed(e.to_string()))?;
                all_matches = sink.matches;
            }
            OutputMode::FilesWithMatches => {
                let mut sink = HasMatchSink::new();
                searcher
                    .search_path(&matcher, path_obj, &mut sink)
                    .map_err(|e| GrepError::SearchFailed(e.to_string()))?;
                if sink.has_match {
                    files_with_matches.push(search_path.to_string());
                }
            }
            OutputMode::Count => {
                let mut sink = CountSink::new();
                searcher
                    .search_path(&matcher, path_obj, &mut sink)
                    .map_err(|e| GrepError::SearchFailed(e.to_string()))?;
                if sink.count > 0 {
                    file_counts.push(GrepFileCount {
                        file: search_path.to_string(),
                        count: sink.count,
                    });
                }
            }
        }
    } else {
        // Parallel directory search
        let walker = WalkBuilder::new(search_path)
            .hidden(false)
            .git_ignore(true)
            .build_parallel();

        let max_items = args.head_limit.unwrap_or(usize::MAX);
        let state = Arc::new(ParallelGrepState::new(max_items));
        let matcher = Arc::new(matcher);
        let glob_set = Arc::new(glob_set);
        let file_type = args.file_type.clone();
        let line_numbers = args.line_numbers.unwrap_or(true);

        walker.run(|| {
            let state = state.clone();
            let matcher = matcher.clone();
            let glob_set = glob_set.clone();
            let file_type = file_type.clone();
            let mut thread_searcher = searcher_builder.build();

            Box::new(move |result| {
                if state.limit_reached.load(Ordering::Relaxed) {
                    return WalkState::Quit;
                }

                let entry = match result {
                    Ok(entry) => entry,
                    Err(_) => return WalkState::Continue,
                };

                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    return WalkState::Continue;
                }

                if !should_include_file(entry.path(), &file_type, &glob_set) {
                    return WalkState::Continue;
                }

                match output_mode {
                    OutputMode::Content => {
                        let mut sink =
                            MatchCollectorSink::with_max_results(entry.path(), line_numbers, None);
                        if thread_searcher
                            .search_path(&*matcher, entry.path(), &mut sink)
                            .is_ok()
                            && !sink.matches.is_empty()
                        {
                            let new_count = state
                                .total_items
                                .fetch_add(sink.matches.len(), Ordering::SeqCst)
                                + sink.matches.len();
                            if new_count >= state.max_items {
                                state.limit_reached.store(true, Ordering::Release);
                            }
                            if let Ok(mut matches) = state.matches.lock() {
                                matches.extend(sink.matches);
                            }
                        }
                    }
                    OutputMode::FilesWithMatches => {
                        let mut sink = HasMatchSink::new();
                        if thread_searcher
                            .search_path(&*matcher, entry.path(), &mut sink)
                            .is_ok()
                            && sink.has_match
                        {
                            let new_count = state.total_items.fetch_add(1, Ordering::SeqCst) + 1;
                            if new_count >= state.max_items {
                                state.limit_reached.store(true, Ordering::Release);
                            }
                            if let Ok(mut files) = state.files_with_matches.lock() {
                                files.push(entry.path().to_string_lossy().to_string());
                            }
                        }
                    }
                    OutputMode::Count => {
                        let mut sink = CountSink::new();
                        if thread_searcher
                            .search_path(&*matcher, entry.path(), &mut sink)
                            .is_ok()
                            && sink.count > 0
                        {
                            let new_count = state.total_items.fetch_add(1, Ordering::SeqCst) + 1;
                            if new_count >= state.max_items {
                                state.limit_reached.store(true, Ordering::Release);
                            }
                            if let Ok(mut counts) = state.file_counts.lock() {
                                counts.push(GrepFileCount {
                                    file: entry.path().to_string_lossy().to_string(),
                                    count: sink.count,
                                });
                            }
                        }
                    }
                }

                WalkState::Continue
            })
        });

        all_matches = state.matches.lock().unwrap().clone();
        files_with_matches = state.files_with_matches.lock().unwrap().clone();
        file_counts = state.file_counts.lock().unwrap().clone();

        all_matches.sort_by(|a, b| a.file.cmp(&b.file));
        files_with_matches.sort();
        file_counts.sort_by(|a, b| a.file.cmp(&b.file));

        if let Some(limit) = args.head_limit {
            all_matches.truncate(limit);
            files_with_matches.truncate(limit);
            file_counts.truncate(limit);
        }
    }

    let match_count = match output_mode {
        OutputMode::Content => all_matches.len(),
        OutputMode::FilesWithMatches => files_with_matches.len(),
        OutputMode::Count => file_counts.iter().map(|fc| fc.count).sum(),
    };

    let display_meta =
        ToolDisplayMeta::grep(args.pattern.clone(), search_path.to_string(), match_count);
    let meta = display_meta.into_meta();

    match output_mode {
        OutputMode::Content => Ok(GrepOutput::Content(GrepContentOutput {
            matches: all_matches,
            total_matches: match_count,
            _meta: meta,
        })),
        OutputMode::FilesWithMatches => Ok(GrepOutput::Files(GrepFilesOutput {
            files: files_with_matches,
            count: match_count,
            _meta: meta,
        })),
        OutputMode::Count => Ok(GrepOutput::Count(GrepCountOutput {
            counts: file_counts,
            total: match_count,
            _meta: meta,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_dir = temp_dir.path();

        fs::write(
            test_dir.join("test.rs"),
            "fn main() {\n    println!(\"Hello, world!\");\n    let x = 42;\n}",
        )
        .unwrap();
        fs::write(
            test_dir.join("script.py"),
            "def hello():\n    print(\"Hello, world!\")\n    x = 42\n",
        )
        .unwrap();
        fs::write(
            test_dir.join("app.js"),
            "function hello() {\n    console.log(\"Hello, world!\");\n    const x = 42;\n}",
        )
        .unwrap();

        temp_dir
    }

    #[tokio::test]
    async fn test_file_type_filtering() {
        let temp_dir = create_test_dir();
        let test_path = temp_dir.path().to_str().unwrap();

        let args = GrepInput {
            pattern: "hello".to_string(),
            path: Some(test_path.to_string()),
            glob: None,
            file_type: Some("rust".to_string()),
            output_mode: Some(OutputMode::Content),
            case_insensitive: Some(true),
            line_numbers: Some(true),
            context_before: None,
            context_after: None,
            context_around: None,
            head_limit: None,
            multiline: None,
        };

        let result = perform_grep(args).await.expect("Failed to perform grep");

        match result {
            GrepOutput::Content(content) => {
                assert!(content.total_matches > 0);
                // Should only find matches in .rs files
                assert!(content.matches.iter().all(|m| m.file.contains("test.rs")));
            }
            _ => panic!("Expected Content output"),
        }
    }

    #[tokio::test]
    async fn test_glob_filtering() {
        let temp_dir = create_test_dir();
        let test_path = temp_dir.path().to_str().unwrap();

        let args = GrepInput {
            pattern: "hello".to_string(),
            path: Some(test_path.to_string()),
            glob: Some("*.py".to_string()),
            file_type: None,
            output_mode: Some(OutputMode::Content),
            case_insensitive: Some(true),
            line_numbers: Some(true),
            context_before: None,
            context_after: None,
            context_around: None,
            head_limit: None,
            multiline: None,
        };

        let result = perform_grep(args).await.expect("Failed to perform grep");

        match result {
            GrepOutput::Content(content) => {
                assert!(content.total_matches > 0);
                // Should only find matches in .py files
                assert!(content.matches.iter().all(|m| m.file.contains("script.py")));
            }
            _ => panic!("Expected Content output"),
        }
    }

    #[tokio::test]
    async fn test_files_with_matches_output() {
        let temp_dir = create_test_dir();
        let test_path = temp_dir.path().to_str().unwrap();

        let args = GrepInput {
            pattern: "hello".to_string(),
            path: Some(test_path.to_string()),
            glob: None,
            file_type: None,
            output_mode: Some(OutputMode::FilesWithMatches),
            case_insensitive: Some(true),
            line_numbers: None,
            context_before: None,
            context_after: None,
            context_around: None,
            head_limit: None,
            multiline: None,
        };

        let result = perform_grep(args).await.expect("Failed to perform grep");

        match result {
            GrepOutput::Files(files) => {
                assert!(files.count >= 2); // At least python and js files
                assert!(files.files.iter().any(|f| f.contains(".py")));
                assert!(files.files.iter().any(|f| f.contains(".js")));
            }
            _ => panic!("Expected Files output"),
        }
    }

    #[tokio::test]
    async fn test_count_output() {
        let temp_dir = create_test_dir();
        let test_path = temp_dir.path().to_str().unwrap();

        let args = GrepInput {
            pattern: "hello".to_string(),
            path: Some(test_path.to_string()),
            glob: None,
            file_type: None,
            output_mode: Some(OutputMode::Count),
            case_insensitive: Some(true),
            line_numbers: None,
            context_before: None,
            context_after: None,
            context_around: None,
            head_limit: None,
            multiline: None,
        };

        let result = perform_grep(args).await.expect("Failed to perform grep");

        match result {
            GrepOutput::Count(count) => {
                assert!(count.counts.len() >= 2);
                assert!(count.total >= 2);
                assert!(count.counts.iter().all(|fc| fc.count > 0));
            }
            _ => panic!("Expected Count output"),
        }
    }

    #[tokio::test]
    async fn test_head_limit() {
        let temp_dir = create_test_dir();
        let test_path = temp_dir.path().to_str().unwrap();

        let args = GrepInput {
            pattern: "hello".to_string(),
            path: Some(test_path.to_string()),
            glob: None,
            file_type: None,
            output_mode: Some(OutputMode::Content),
            case_insensitive: Some(true),
            line_numbers: Some(true),
            context_before: None,
            context_after: None,
            context_around: None,
            head_limit: Some(1),
            multiline: None,
        };

        let result = perform_grep(args).await.expect("Failed to perform grep");

        match result {
            GrepOutput::Content(content) => {
                assert!(content.total_matches <= 1);
            }
            _ => panic!("Expected Content output"),
        }
    }

    #[tokio::test]
    async fn test_multiline_mode() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_path = temp_dir.path().join("multiline.txt");

        // Create a file with content that spans multiple lines
        fs::write(&test_path, "start\nmiddle content\nend").unwrap();

        // Test multiline pattern that matches across lines
        let args = GrepInput {
            pattern: r"start.*end".to_string(),
            path: Some(test_path.to_str().unwrap().to_string()),
            glob: None,
            file_type: None,
            output_mode: Some(OutputMode::Content),
            case_insensitive: None,
            line_numbers: Some(true),
            context_before: None,
            context_after: None,
            context_around: None,
            head_limit: None,
            multiline: Some(true),
        };

        let result = perform_grep(args).await.expect("Failed to perform grep");

        match result {
            GrepOutput::Content(content) => {
                // With multiline mode, it should match the pattern spanning multiple lines
                assert!(content.total_matches > 0);
            }
            _ => panic!("Expected Content output"),
        }
    }

    #[tokio::test]
    async fn test_context_lines() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_path = temp_dir.path().join("context.txt");

        fs::write(&test_path, "line 1\nline 2\ntarget\nline 4\nline 5").unwrap();

        let args = GrepInput {
            pattern: "target".to_string(),
            path: Some(test_path.to_str().unwrap().to_string()),
            glob: None,
            file_type: None,
            output_mode: Some(OutputMode::Content),
            case_insensitive: None,
            line_numbers: Some(true),
            context_before: Some(1),
            context_after: Some(1),
            context_around: None,
            head_limit: None,
            multiline: None,
        };

        let result = perform_grep(args).await.expect("Failed to perform grep");

        match result {
            GrepOutput::Content(content) => {
                assert!(content.total_matches > 0);
                // Check that context is present
                assert!(content.matches[0].before_context.is_some());
                assert!(content.matches[0].after_context.is_some());
                let before = content.matches[0].before_context.as_ref().unwrap();
                let after = content.matches[0].after_context.as_ref().unwrap();
                assert_eq!(before.len(), 1);
                assert_eq!(after.len(), 1);
            }
            _ => panic!("Expected Content output"),
        }
    }
}

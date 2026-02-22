pub mod common;

use crate::coding::error::GrepError;
use mcp_utils::display_meta::{ToolDisplayMeta, ToolResultMeta, basename};
use aether_lspd::extensions_for_alias as extensions_for_type;
use common::{CountSink, HasMatchSink, MatchCollectorSink, MatchData, OutputMode};
use globset::{Glob, GlobSetBuilder};
use grep::{
    regex::{RegexMatcher, RegexMatcherBuilder},
    searcher::{BinaryDetection, Searcher, SearcherBuilder},
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
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub _meta: Option<ToolResultMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrepFilesOutput {
    /// Files containing matches
    pub files: Vec<String>,
    /// Number of files with matches
    pub count: usize,
    /// Display metadata for human-friendly rendering
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub _meta: Option<ToolResultMeta>,
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
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub _meta: Option<ToolResultMeta>,
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
    file_type: Option<&String>,
    glob_pattern: Option<&globset::GlobSet>,
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
    let glob_set = build_glob_set(args.glob.as_deref())?;

    let matcher = build_matcher(&args.pattern, args.case_insensitive, args.multiline)?;

    let output_mode = args.output_mode.unwrap_or(OutputMode::Content);
    let line_numbers = args.line_numbers.unwrap_or(true);
    let searcher_builder = build_searcher(&args);

    let search_path = args.path.as_deref().unwrap_or(".");
    let path_obj = Path::new(search_path);

    let config = SearchConfig {
        output_mode,
        line_numbers,
        file_type: args.file_type.as_ref(),
    };

    let results = if path_obj.is_file() {
        search_single_file(
            path_obj,
            search_path,
            args.head_limit,
            &matcher,
            glob_set.as_ref(),
            &config,
            &searcher_builder,
        )?
    } else {
        search_directory(
            search_path,
            &args,
            glob_set,
            matcher,
            &config,
            &searcher_builder,
        )
    };

    Ok(build_grep_output(
        results,
        output_mode,
        &args.pattern,
        search_path,
    ))
}

/// Common search parameters shared across single-file and directory search paths.
struct SearchConfig<'a> {
    output_mode: OutputMode,
    line_numbers: bool,
    file_type: Option<&'a String>,
}

/// Collected search results across all searched files.
struct SearchResults {
    matches: Vec<MatchData>,
    files_with_matches: Vec<String>,
    file_counts: Vec<GrepFileCount>,
}

impl SearchResults {
    fn empty() -> Self {
        Self {
            matches: Vec::new(),
            files_with_matches: Vec::new(),
            file_counts: Vec::new(),
        }
    }
}

fn build_glob_set(glob_pattern: Option<&str>) -> Result<Option<globset::GlobSet>, GrepError> {
    let Some(glob_pattern) = glob_pattern else {
        return Ok(None);
    };
    let mut builder = GlobSetBuilder::new();
    builder.add(
        Glob::new(glob_pattern).map_err(|e| GrepError::InvalidGlobPattern {
            pattern: glob_pattern.to_owned(),
            reason: e.to_string(),
        })?,
    );
    builder
        .build()
        .map(Some)
        .map_err(|e| GrepError::GlobSetBuildFailed(e.to_string()))
}

fn build_matcher(
    pattern: &str,
    case_insensitive: Option<bool>,
    multiline: Option<bool>,
) -> Result<RegexMatcher, GrepError> {
    let mut matcher_builder = RegexMatcherBuilder::new();
    matcher_builder.case_insensitive(case_insensitive.unwrap_or(false));

    if multiline.unwrap_or(false) {
        matcher_builder.multi_line(true).dot_matches_new_line(true);
    }

    matcher_builder
        .build(pattern)
        .map_err(|e| GrepError::InvalidRegex(e.to_string()))
}

fn build_searcher(args: &GrepInput) -> SearcherBuilder {
    let (context_before, context_after) = if let Some(around) = args.context_around {
        (around, around)
    } else {
        (
            args.context_before.unwrap_or(0),
            args.context_after.unwrap_or(0),
        )
    };

    let mut builder = SearcherBuilder::new();
    builder
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(args.line_numbers.unwrap_or(true))
        .before_context(context_before as usize)
        .after_context(context_after as usize);

    if args.multiline.unwrap_or(false) {
        builder.multi_line(true);
    }

    builder
}

fn search_single_file(
    path_obj: &Path,
    search_path: &str,
    head_limit: Option<usize>,
    matcher: &RegexMatcher,
    glob_set: Option<&globset::GlobSet>,
    config: &SearchConfig<'_>,
    searcher_builder: &SearcherBuilder,
) -> Result<SearchResults, GrepError> {
    if !should_include_file(path_obj, config.file_type, glob_set) {
        return Ok(SearchResults::empty());
    }

    let mut searcher = searcher_builder.build();
    let mut results = SearchResults::empty();

    match config.output_mode {
        OutputMode::Content => {
            let mut sink =
                MatchCollectorSink::with_max_results(path_obj, config.line_numbers, head_limit);
            searcher
                .search_path(matcher, path_obj, &mut sink)
                .map_err(|e| GrepError::SearchFailed(e.to_string()))?;
            results.matches = sink.matches;
        }
        OutputMode::FilesWithMatches => {
            let mut sink = HasMatchSink::new();
            searcher
                .search_path(matcher, path_obj, &mut sink)
                .map_err(|e| GrepError::SearchFailed(e.to_string()))?;
            if sink.has_match {
                results.files_with_matches.push(search_path.to_string());
            }
        }
        OutputMode::Count => {
            let mut sink = CountSink::new();
            searcher
                .search_path(matcher, path_obj, &mut sink)
                .map_err(|e| GrepError::SearchFailed(e.to_string()))?;
            if sink.count > 0 {
                results.file_counts.push(GrepFileCount {
                    file: search_path.to_string(),
                    count: sink.count,
                });
            }
        }
    }

    Ok(results)
}

fn search_directory(
    search_path: &str,
    args: &GrepInput,
    glob_set: Option<globset::GlobSet>,
    matcher: RegexMatcher,
    config: &SearchConfig<'_>,
    searcher_builder: &SearcherBuilder,
) -> SearchResults {
    let walker = WalkBuilder::new(search_path)
        .hidden(false)
        .git_ignore(true)
        .build_parallel();

    let max_items = args.head_limit.unwrap_or(usize::MAX);
    let state = Arc::new(ParallelGrepState::new(max_items));
    let matcher = Arc::new(matcher);
    let glob_set = Arc::new(glob_set);
    let file_type = args.file_type.clone();
    let output_mode = config.output_mode;
    let line_numbers = config.line_numbers;

    walker.run(|| {
        let state = state.clone();
        let matcher = matcher.clone();
        let glob_set = glob_set.clone();
        let file_type = file_type.clone();
        let mut thread_searcher = searcher_builder.build();

        Box::new(move |result| {
            let filter = FileFilter {
                glob_set: glob_set.as_ref().as_ref(),
                file_type: file_type.as_ref(),
            };
            search_directory_entry(
                result,
                &state,
                &matcher,
                &filter,
                &mut thread_searcher,
                output_mode,
                line_numbers,
            )
        })
    });

    let mut results = SearchResults {
        matches: state.matches.lock().unwrap().clone(),
        files_with_matches: state.files_with_matches.lock().unwrap().clone(),
        file_counts: state.file_counts.lock().unwrap().clone(),
    };

    results.matches.sort_by(|a, b| a.file.cmp(&b.file));
    results.files_with_matches.sort();
    results.file_counts.sort_by(|a, b| a.file.cmp(&b.file));

    if let Some(limit) = args.head_limit {
        results.matches.truncate(limit);
        results.files_with_matches.truncate(limit);
        results.file_counts.truncate(limit);
    }

    results
}

/// Filter criteria for file selection during directory walks.
struct FileFilter<'a> {
    glob_set: Option<&'a globset::GlobSet>,
    file_type: Option<&'a String>,
}

fn search_directory_entry(
    result: std::result::Result<ignore::DirEntry, ignore::Error>,
    state: &ParallelGrepState,
    matcher: &RegexMatcher,
    filter: &FileFilter<'_>,
    searcher: &mut Searcher,
    output_mode: OutputMode,
    line_numbers: bool,
) -> WalkState {
    if state.limit_reached.load(Ordering::Relaxed) {
        return WalkState::Quit;
    }

    let Ok(entry) = result else {
        return WalkState::Continue;
    };

    if !entry.file_type().is_some_and(|ft| ft.is_file()) {
        return WalkState::Continue;
    }

    if !should_include_file(entry.path(), filter.file_type, filter.glob_set) {
        return WalkState::Continue;
    }

    match output_mode {
        OutputMode::Content => {
            let mut sink = MatchCollectorSink::with_max_results(entry.path(), line_numbers, None);
            if searcher
                .search_path(matcher, entry.path(), &mut sink)
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
            if searcher
                .search_path(matcher, entry.path(), &mut sink)
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
            if searcher
                .search_path(matcher, entry.path(), &mut sink)
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
}

fn build_grep_output(
    results: SearchResults,
    output_mode: OutputMode,
    pattern: &str,
    search_path: &str,
) -> GrepOutput {
    let match_count = match output_mode {
        OutputMode::Content => results.matches.len(),
        OutputMode::FilesWithMatches => results.files_with_matches.len(),
        OutputMode::Count => results.file_counts.iter().map(|fc| fc.count).sum(),
    };

    let display_meta = ToolDisplayMeta::new(
        "Grep",
        format!(
            "'{}' in {} ({} matches)",
            pattern,
            basename(search_path),
            match_count
        ),
    );
    let meta = Some(display_meta.into());

    match output_mode {
        OutputMode::Content => GrepOutput::Content(GrepContentOutput {
            matches: results.matches,
            total_matches: match_count,
            _meta: meta,
        }),
        OutputMode::FilesWithMatches => GrepOutput::Files(GrepFilesOutput {
            files: results.files_with_matches,
            count: match_count,
            _meta: meta,
        }),
        OutputMode::Count => GrepOutput::Count(GrepCountOutput {
            counts: results.file_counts,
            total: match_count,
            _meta: meta,
        }),
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

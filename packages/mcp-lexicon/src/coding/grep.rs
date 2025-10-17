use globset::{Glob, GlobSetBuilder};
use grep::{
    regex::RegexMatcherBuilder,
    searcher::{BinaryDetection, SearcherBuilder},
};
use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::common::{CountSink, HasMatchSink, MatchCollectorSink, MatchData, OutputMode};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepContentOutput {
    /// Matching lines with context
    pub matches: Vec<MatchData>,
    /// Total number of matches
    pub total_matches: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepFilesOutput {
    /// Files containing matches
    pub files: Vec<String>,
    /// Number of files with matches
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepFileCount {
    pub file: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepCountOutput {
    /// Match counts per file
    pub counts: Vec<GrepFileCount>,
    /// Total matches across all files
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum GrepOutput {
    Content(GrepContentOutput),
    Files(GrepFilesOutput),
    Count(GrepCountOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
    /// Output mode: "content", "files_with_matches", or "count"
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
            let extensions = get_file_extensions_for_type(ftype);
            return extensions.contains(&ext);
        }
        return false;
    }

    true
}

pub async fn perform_grep(args: GrepInput) -> Result<GrepOutput, String> {
    // Build glob set if glob pattern is provided
    let glob_set = if let Some(glob_pattern) = &args.glob {
        let mut builder = GlobSetBuilder::new();
        builder.add(
            Glob::new(glob_pattern)
                .map_err(|e| format!("Invalid glob pattern '{}': {}", glob_pattern, e))?,
        );
        Some(builder.build().map_err(|e| format!("Failed to build glob set: {}", e))?)
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
        .map_err(|e| format!("Invalid regex pattern: {}", e))?;

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
            return match output_mode {
                OutputMode::Content => Ok(GrepOutput::Content(GrepContentOutput {
                    matches: vec![],
                    total_matches: 0,
                })),
                OutputMode::FilesWithMatches => Ok(GrepOutput::Files(GrepFilesOutput {
                    files: vec![],
                    count: 0,
                })),
                OutputMode::Count => Ok(GrepOutput::Count(GrepCountOutput {
                    counts: vec![],
                    total: 0,
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
                    .map_err(|e| format!("Search error: {}", e))?;
                all_matches = sink.matches;
            }
            OutputMode::FilesWithMatches => {
                let mut sink = HasMatchSink::new();
                searcher
                    .search_path(&matcher, path_obj, &mut sink)
                    .map_err(|e| format!("Search error: {}", e))?;
                if sink.has_match {
                    files_with_matches.push(search_path.to_string());
                }
            }
            OutputMode::Count => {
                let mut sink = CountSink::new();
                searcher
                    .search_path(&matcher, path_obj, &mut sink)
                    .map_err(|e| format!("Search error: {}", e))?;
                if sink.count > 0 {
                    file_counts.push(GrepFileCount {
                        file: search_path.to_string(),
                        count: sink.count,
                    });
                }
            }
        }
    } else {
        // Directory search
        let walker = WalkBuilder::new(search_path)
            .hidden(false)
            .git_ignore(true)
            .build();

        let mut total_items = 0;
        let max_items = args.head_limit.unwrap_or(usize::MAX);

        for result in walker {
            // Check if we've reached the limit
            if total_items >= max_items {
                break;
            }

            match result {
                Ok(entry) => {
                    if entry.file_type().is_some_and(|ft| ft.is_file()) {
                        // Check file filtering
                        if !should_include_file(entry.path(), &args.file_type, &glob_set) {
                            continue;
                        }

                        match output_mode {
                            OutputMode::Content => {
                                let remaining = max_items.saturating_sub(total_items);
                                let mut sink = MatchCollectorSink::with_max_results(
                                    entry.path(),
                                    args.line_numbers.unwrap_or(true),
                                    Some(remaining),
                                );
                                if searcher.search_path(&matcher, entry.path(), &mut sink).is_ok() {
                                    total_items += sink.matches.len();
                                    all_matches.extend(sink.matches);
                                }
                            }
                            OutputMode::FilesWithMatches => {
                                let mut sink = HasMatchSink::new();
                                if searcher.search_path(&matcher, entry.path(), &mut sink).is_ok()
                                    && sink.has_match
                                {
                                    files_with_matches
                                        .push(entry.path().to_string_lossy().to_string());
                                    total_items += 1;
                                }
                            }
                            OutputMode::Count => {
                                let mut sink = CountSink::new();
                                if searcher.search_path(&matcher, entry.path(), &mut sink).is_ok()
                                    && sink.count > 0
                                {
                                    file_counts.push(GrepFileCount {
                                        file: entry.path().to_string_lossy().to_string(),
                                        count: sink.count,
                                    });
                                    total_items += 1;
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
        OutputMode::Content => {
            let total_matches = all_matches.len();
            Ok(GrepOutput::Content(GrepContentOutput {
                matches: all_matches,
                total_matches,
            }))
        }
        OutputMode::FilesWithMatches => {
            let count = files_with_matches.len();
            Ok(GrepOutput::Files(GrepFilesOutput {
                files: files_with_matches,
                count,
            }))
        }
        OutputMode::Count => {
            let total: usize = file_counts.iter().map(|fc| fc.count).sum();
            Ok(GrepOutput::Count(GrepCountOutput {
                counts: file_counts,
                total,
            }))
        }
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

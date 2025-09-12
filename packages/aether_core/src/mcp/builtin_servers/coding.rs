use grep::matcher::Matcher;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub pattern: String,
    pub path: Option<String>,
    pub case_insensitive: Option<bool>,
    pub line_numbers: Option<bool>,
    pub context: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchFilesArgs {
    pub pattern: String,
    pub path: Option<String>,
    pub case_insensitive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchInFileArgs {
    pub pattern: String,
    pub file_path: String,
    pub case_insensitive: Option<bool>,
    pub line_numbers: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FindFilesArgs {
    pub filename_pattern: String,
    pub path: Option<String>,
    pub case_insensitive: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CodingMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for CodingMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "coding-mcp".to_string(),
                version: "0.1.0".to_string(),
            },
            instructions: Some(
                "A coding MCP with grep-powered search server with advanced text search capabilities".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl CodingMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Search for a pattern in files using ripgrep")]
    pub async fn search(&self, request: Parameters<SearchArgs>) -> String {
        let Parameters(args) = request;

        match self.perform_search(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Search error: {}", e),
        }
    }

    #[tool(description = "Find files containing a pattern")]
    pub async fn search_files(&self, request: Parameters<SearchFilesArgs>) -> String {
        let Parameters(args) = request;

        match self.find_files_with_pattern(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Search files error: {}", e),
        }
    }

    #[tool(description = "Search for a pattern in a specific file")]
    pub async fn search_in_file(&self, request: Parameters<SearchInFileArgs>) -> String {
        let Parameters(args) = request;

        match self.search_single_file(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Search in file error: {}", e),
        }
    }

    #[tool(
        description = "Find files by filename pattern (supports wildcards like *.rs, main.*, etc.)"
    )]
    pub async fn find_files(&self, request: Parameters<FindFilesArgs>) -> String {
        let Parameters(args) = request;

        match self.find_files_by_name(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Find files error: {}", e),
        }
    }

    async fn perform_search(&self, args: SearchArgs) -> Result<serde_json::Value, String> {
        let matcher = self.build_matcher(&args.pattern, args.case_insensitive.unwrap_or(false))?;
        let search_path = args.path.as_deref().unwrap_or(".");
        let mut matches = Vec::new();

        // Use ignore crate for proper file walking
        let walker = WalkBuilder::new(search_path)
            .hidden(false) // Include hidden files by default
            .git_ignore(true)
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        if let Ok(file_matches) = self.search_file_with_searcher(
                            &matcher,
                            entry.path(),
                            args.line_numbers.unwrap_or(true),
                        ) {
                            matches.extend(file_matches);
                        }
                    }
                }
                Err(_) => continue, // Skip errors
            }
        }

        Ok(serde_json::json!({
            "status": "success",
            "pattern": args.pattern,
            "path": search_path,
            "matches": matches,
            "match_count": matches.len()
        }))
    }

    async fn find_files_with_pattern(
        &self,
        args: SearchFilesArgs,
    ) -> Result<serde_json::Value, String> {
        let matcher = self.build_matcher(&args.pattern, args.case_insensitive.unwrap_or(false))?;
        let search_path = args.path.as_deref().unwrap_or(".");
        let mut files_with_matches = Vec::new();

        // Use ignore crate for proper file walking
        let walker = WalkBuilder::new(search_path)
            .hidden(false) // Include hidden files by default
            .git_ignore(true)
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        if let Ok(has_match) =
                            self.file_has_matches_with_searcher(&matcher, entry.path())
                        {
                            if has_match {
                                files_with_matches.push(entry.path().to_string_lossy().to_string());
                            }
                        }
                    }
                }
                Err(_) => continue, // Skip errors
            }
        }

        Ok(serde_json::json!({
            "status": "success",
            "pattern": args.pattern,
            "path": search_path,
            "files": files_with_matches,
            "file_count": files_with_matches.len()
        }))
    }

    async fn search_single_file(
        &self,
        args: SearchInFileArgs,
    ) -> Result<serde_json::Value, String> {
        if !Path::new(&args.file_path).exists() {
            return Err(format!("File does not exist: {}", args.file_path));
        }

        let matcher = self.build_matcher(&args.pattern, args.case_insensitive.unwrap_or(false))?;
        let matches = self.search_file_with_searcher(
            &matcher,
            Path::new(&args.file_path),
            args.line_numbers.unwrap_or(true),
        )?;

        Ok(serde_json::json!({
            "status": "success",
            "pattern": args.pattern,
            "file": args.file_path,
            "matches": matches,
            "match_count": matches.len()
        }))
    }

    fn build_matcher(&self, pattern: &str, case_insensitive: bool) -> Result<impl Matcher, String> {
        RegexMatcherBuilder::new()
            .case_insensitive(case_insensitive)
            .build(pattern)
            .map_err(|e| format!("Invalid regex pattern: {}", e))
    }

    fn search_file_with_searcher<M: Matcher>(
        &self,
        matcher: &M,
        file_path: &Path,
        line_numbers: bool,
    ) -> Result<Vec<String>, String> {
        let mut matches = Vec::new();
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .build();

        // Create a sink to collect matches
        let mut sink = MatchSink::new(file_path, line_numbers, &mut matches);

        searcher
            .search_path(matcher, file_path, &mut sink)
            .map_err(|e| format!("Search error in {}: {}", file_path.display(), e))?;

        Ok(matches)
    }

    fn file_has_matches_with_searcher<M: Matcher>(
        &self,
        matcher: &M,
        file_path: &Path,
    ) -> Result<bool, String> {
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .build();

        let mut has_match = false;
        let mut sink = HasMatchSink::new(&mut has_match);

        searcher
            .search_path(matcher, file_path, &mut sink)
            .map_err(|e| format!("Search error in {}: {}", file_path.display(), e))?;

        Ok(has_match)
    }

    async fn find_files_by_name(&self, args: FindFilesArgs) -> Result<serde_json::Value, String> {
        let search_path = args.path.as_deref().unwrap_or(".");
        let mut matching_files = Vec::new();
        let case_insensitive = args.case_insensitive.unwrap_or(false);

        // Use ignore crate for proper file walking
        let walker = WalkBuilder::new(search_path)
            .hidden(false) // Include hidden files by default
            .git_ignore(true)
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        if let Some(filename) = entry.path().file_name() {
                            let filename_str = filename.to_string_lossy();
                            let pattern = &args.filename_pattern;

                            let matches = if case_insensitive {
                                self.pattern_matches(
                                    &filename_str.to_lowercase(),
                                    &pattern.to_lowercase(),
                                )
                            } else {
                                self.pattern_matches(&filename_str, pattern)
                            };

                            if matches {
                                matching_files.push(entry.path().to_string_lossy().to_string());
                            }
                        }
                    }
                }
                Err(_) => continue, // Skip errors
            }
        }

        Ok(serde_json::json!({
            "status": "success",
            "filename_pattern": args.filename_pattern,
            "path": search_path,
            "files": matching_files,
            "file_count": matching_files.len()
        }))
    }

    fn pattern_matches(&self, filename: &str, pattern: &str) -> bool {
        // Simple pattern matching - support * as wildcard
        if pattern.contains('*') {
            // Convert glob pattern to regex
            let regex_pattern = pattern.replace(".", "\\.").replace("*", ".*");

            if let Ok(regex) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
                return regex.is_match(filename);
            }
        }

        // Exact match or substring match
        filename == pattern || filename.contains(pattern)
    }
}

// Sink for collecting matches
struct MatchSink<'a> {
    file_path: &'a Path,
    line_numbers: bool,
    matches: &'a mut Vec<String>,
}

impl<'a> MatchSink<'a> {
    fn new(file_path: &'a Path, line_numbers: bool, matches: &'a mut Vec<String>) -> Self {
        Self {
            file_path,
            line_numbers,
            matches,
        }
    }
}

impl<'a> Sink for MatchSink<'a> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        let line_str = std::str::from_utf8(mat.bytes()).unwrap_or("<invalid utf8>");
        let match_str = if self.line_numbers {
            format!(
                "{}:{}:{}",
                self.file_path.display(),
                mat.line_number().unwrap_or(0),
                line_str
            )
        } else {
            format!("{}:{}", self.file_path.display(), line_str)
        };
        self.matches.push(match_str);
        Ok(true) // Continue searching
    }
}

// Sink for checking if file has matches
struct HasMatchSink<'a> {
    has_match: &'a mut bool,
}

impl<'a> HasMatchSink<'a> {
    fn new(has_match: &'a mut bool) -> Self {
        Self { has_match }
    }
}

impl<'a> Sink for HasMatchSink<'a> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        _mat: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        *self.has_match = true;
        Ok(false) // Stop searching after first match
    }
}

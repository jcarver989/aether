use grep::searcher::{Sink, SinkMatch};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum OutputMode {
    /// Return matching lines with file paths and line numbers
    #[serde(rename = "matches")]
    Matches,
    /// Return only file paths that contain matches
    #[serde(rename = "files_only")]
    FilesOnly,
}

pub struct MatchCollectorSink {
    file_path: std::path::PathBuf,
    line_numbers: bool,
    max_results: Option<usize>,
    pub matches: Vec<String>,
}

impl MatchCollectorSink {
    pub fn new(file_path: &Path, line_numbers: bool) -> Self {
        Self {
            file_path: file_path.to_path_buf(),
            line_numbers,
            max_results: None,
            matches: Vec::new(),
        }
    }

    pub fn with_max_results(file_path: &Path, line_numbers: bool, max_results: Option<usize>) -> Self {
        Self {
            file_path: file_path.to_path_buf(),
            line_numbers,
            max_results,
            matches: Vec::new(),
        }
    }
}

impl Sink for MatchCollectorSink {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        // Check if we've hit the max results limit
        if let Some(max) = self.max_results {
            if self.matches.len() >= max {
                return Ok(false); // Stop searching
            }
        }

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
        Ok(true)
    }
}

pub struct HasMatchSink {
    pub has_match: bool,
}

impl HasMatchSink {
    pub fn new() -> Self {
        Self { has_match: false }
    }
}

impl Sink for HasMatchSink {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        _mat: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        self.has_match = true;
        Ok(false)
    }
}
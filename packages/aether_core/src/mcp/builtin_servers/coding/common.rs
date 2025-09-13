use grep::searcher::{Sink, SinkMatch};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum OutputMode {
    #[serde(rename = "matches")]
    Matches,
    #[serde(rename = "files_only")]
    FilesOnly,
}

pub struct MatchCollectorSink {
    file_path: std::path::PathBuf,
    line_numbers: bool,
    pub matches: Vec<String>,
}

impl MatchCollectorSink {
    pub fn new(file_path: &Path, line_numbers: bool) -> Self {
        Self {
            file_path: file_path.to_path_buf(),
            line_numbers,
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
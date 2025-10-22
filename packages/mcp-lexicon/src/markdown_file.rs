use futures::future::join_all;
use serde::de::DeserializeOwned;
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

/// Represents a parsed markdown file with optional frontmatter
#[derive(Debug, Clone)]
pub struct MarkdownFile<T: DeserializeOwned> {
    /// Name of the file (derived from filename without extension)
    pub name: String,
    /// Parsed frontmatter (if present)
    pub frontmatter: Option<T>,
    /// The content after frontmatter
    pub content: String,
}

impl<T: DeserializeOwned + Send + 'static> MarkdownFile<T> {
    /// List all markdown files in a directory
    pub fn list(dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, io::Error> {
        let paths: Vec<_> = fs::read_dir(dir)?
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                (path.extension().and_then(|s| s.to_str()) == Some("md")).then_some(path)
            })
            .collect();

        Ok(paths)
    }

    /// Load a single markdown file from a path
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ParseError> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(ParseError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path.display()),
            )));
        }

        let name = path
            .file_stem()
            .ok_or(ParseError::InvalidFilename)?
            .to_string_lossy()
            .to_string();

        let content = fs::read_to_string(path)?;
        let (frontmatter, content) = split_frontmatter::<T>(&content);

        Ok(MarkdownFile {
            name,
            frontmatter,
            content,
        })
    }

    /// Load all markdown files from a directory
    pub async fn from_dir(dir: &PathBuf) -> Result<Vec<Self>, io::Error> {
        if !dir.exists() {
            tracing::warn!("Directory does not exist: {}", dir.display());
            return Ok(Vec::new());
        }

        let parse_tasks: Vec<_> = Self::list(dir)?
            .into_iter()
            .map(|path| {
                tokio::spawn(async move {
                    let result = (|| -> Result<Self, ParseError> {
                        let name = path
                            .file_stem()
                            .ok_or(ParseError::InvalidFilename)?
                            .to_string_lossy()
                            .to_string();

                        let content = fs::read_to_string(&path)?;
                        let (frontmatter, content) = split_frontmatter::<T>(&content);

                        Ok(MarkdownFile {
                            name,
                            frontmatter,
                            content,
                        })
                    })();
                    (path, result)
                })
            })
            .collect();

        let results = join_all(parse_tasks).await;
        let items = results
            .into_iter()
            .filter_map(|result| match result {
                Ok((_, Ok(item))) => Some(item),
                Ok((path, Err(e))) => {
                    tracing::warn!("Failed to parse {}: {}", path.display(), e);
                    None
                }
                Err(_) => None,
            })
            .collect();

        Ok(items)
    }
}

/// Split YAML frontmatter from markdown content
fn split_frontmatter<T: DeserializeOwned>(content: &str) -> (Option<T>, String) {
    let content = content.trim();

    if !content.starts_with("---") {
        return (None, content.to_string());
    }

    // Find the end of frontmatter (second ---)
    let rest = &content[3..];
    let end_pos = match rest.find("\n---") {
        Some(pos) => pos,
        None => return (None, content.to_string()),
    };

    let frontmatter_str = &rest[..end_pos];
    let template = rest[end_pos + 4..].trim().to_string();

    match serde_yaml::from_str(frontmatter_str) {
        Ok(frontmatter) => (Some(frontmatter), template),
        Err(_) => (None, content.to_string()),
    }
}

#[derive(Debug)]
pub enum ParseError {
    InvalidFilename,
    Io(io::Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidFilename => write!(f, "Invalid filename"),
            ParseError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::Io(e) => Some(e),
            ParseError::InvalidFilename => None,
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(e: io::Error) -> Self {
        ParseError::Io(e)
    }
}

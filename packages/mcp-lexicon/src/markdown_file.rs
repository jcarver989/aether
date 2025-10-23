use futures::future::join_all;
use serde::de::DeserializeOwned;
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

/// Represents a parsed markdown file with optional frontmatter
#[derive(Debug, Clone)]
pub struct MarkdownFile<T: DeserializeOwned> {
    /// Parsed frontmatter (if present)
    pub frontmatter: Option<T>,
    /// The content after frontmatter
    pub content: String,
}

/// Parse a markdown file from a path
fn parse_markdown_file<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<MarkdownFile<T>, ParseError> {
    let raw_content = fs::read_to_string(path)?;
    let (frontmatter, content) = split_frontmatter::<T>(&raw_content);

    Ok(MarkdownFile {
        frontmatter,
        content,
    })
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

        parse_markdown_file(path)
    }

    /// Load all markdown files from a directory
    pub async fn from_dir(dir: &PathBuf) -> Result<Vec<(PathBuf, Self)>, io::Error> {
        if !dir.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Directory not found: {}", dir.display()),
            ));
        }

        if !dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("Not a directory: {}", dir.display()),
            ));
        }

        let parse_tasks: Vec<_> = Self::list(dir)?
            .into_iter()
            .map(|path| {
                tokio::spawn(async move {
                    let path_clone = path.clone();
                    parse_markdown_file(path).map(|f| (path_clone, f))
                })
            })
            .collect();

        let results = join_all(parse_tasks).await;
        let items = results
            .into_iter()
            .filter_map(|result| match result {
                Ok(Ok(item)) => Some(item),
                Ok(Err(e)) => {
                    tracing::warn!("Failed to parse file: {}", e);
                    None
                }
                Err(_) => None,
            })
            .collect();

        Ok(items)
    }

    /// Load all markdown files from nested subdirectories, where each subdirectory
    /// contains a file with the specified filename.
    ///
    /// Flat files in the parent directory are ignored. Only subdirectories containing
    /// the specified filename are processed.
    ///
    /// # Example
    /// ```ignore
    /// // Load from:
    /// //   skills/skill-1/SKILL.md
    /// //   skills/skill-2/SKILL.md
    /// //   skills/flat-file.md      -> ignored (not in a subdirectory)
    /// let skills = MarkdownFile::from_nested_dirs(Path::new("skills"), "SKILL.md").await?;
    /// ```
    pub async fn from_nested_dirs(
        parent_dir: impl AsRef<Path>,
        filename: &str,
    ) -> Result<Vec<(PathBuf, Self)>, io::Error> {
        let parent_dir = parent_dir.as_ref();

        if !parent_dir.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Directory not found: {}", parent_dir.display()),
            ));
        }

        if !parent_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("Not a directory: {}", parent_dir.display()),
            ));
        }

        let subdirs = list_subdirs(parent_dir)?;
        let filename = filename.to_string();
        let parse_tasks: Vec<_> = subdirs
            .into_iter()
            .map(|dir| {
                let filename = filename.clone();
                tokio::spawn(async move {
                    let file_path = dir.join(&filename);
                    parse_markdown_file(&file_path).map(|f| (dir, f))
                })
            })
            .collect();

        let results = join_all(parse_tasks).await;
        let items = results
            .into_iter()
            .filter_map(|result| match result {
                Ok(Ok(item)) => Some(item),
                Ok(Err(e)) => {
                    tracing::debug!("Skipping directory: {}", e);
                    None
                }
                Err(_) => None,
            })
            .collect();

        Ok(items)
    }
}

/// List all subdirectories in a directory
fn list_subdirs(dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, io::Error> {
    let paths: Vec<_> = fs::read_dir(dir)?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            path.is_dir().then_some(path)
        })
        .collect();

    Ok(paths)
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
            ParseError::Io(e) => write!(f, "IO error: {e}"),
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

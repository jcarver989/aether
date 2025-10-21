use rmcp::model::Prompt;
use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path::Path};

/// Frontmatter metadata for a prompt file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFrontmatter {
    /// Description of what the prompt does
    pub description: Option<String>,
}

/// Represents a parsed prompt file
#[derive(Debug, Clone)]
pub struct PromptFile {
    /// Name of the prompt (derived from filename without extension)
    pub name: String,
    /// Metadata from frontmatter
    pub frontmatter: PromptFrontmatter,
    /// The template content (markdown after frontmatter)
    pub template: String,
}

impl PromptFile {
    /// Parse a prompt file from a markdown file path
    ///
    /// Reads the file and parses YAML frontmatter if present.
    pub fn parse(path: &Path) -> Result<PromptFile, ParseError> {
        let name = path
            .file_stem()
            .ok_or(ParseError::InvalidFilename)?
            .to_string_lossy()
            .to_string();

        let content = fs::read_to_string(path)?;
        let content = content.trim();

        if !content.starts_with("---") {
            return Ok(PromptFile {
                name,
                frontmatter: PromptFrontmatter { description: None },
                template: content.to_string(),
            });
        }

        // Find the end of frontmatter (second ---)
        let rest = &content[3..];
        let end_pos = rest
            .find("\n---")
            .ok_or_else(|| ParseError::MissingClosingDelimiter(name.clone()))?;

        let frontmatter_str = &rest[..end_pos];
        let template = rest[end_pos + 4..].trim().to_string();
        let frontmatter: PromptFrontmatter = serde_yaml::from_str(frontmatter_str)?;

        Ok(PromptFile {
            name,
            frontmatter,
            template,
        })
    }
}

#[derive(Debug)]
pub enum ParseError {
    InvalidFilename,
    Io(io::Error),
    MissingClosingDelimiter(String),
    Yaml(serde_yaml::Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidFilename => write!(f, "Invalid filename"),
            ParseError::Io(e) => write!(f, "IO error: {}", e),
            ParseError::MissingClosingDelimiter(name) => {
                write!(f, "Invalid frontmatter in {}: missing closing '---'", name)
            }
            ParseError::Yaml(e) => write!(f, "YAML parse error: {}", e),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::Io(e) => Some(e),
            ParseError::Yaml(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(e: io::Error) -> Self {
        ParseError::Io(e)
    }
}

impl From<serde_yaml::Error> for ParseError {
    fn from(e: serde_yaml::Error) -> Self {
        ParseError::Yaml(e)
    }
}

impl From<&PromptFile> for Prompt {
    fn from(prompt_file: &PromptFile) -> Self {
        Prompt::new(
            prompt_file.name.clone(),
            prompt_file.frontmatter.description.clone(),
            None,
        )
    }
}

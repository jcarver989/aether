//! Language identifier for LSP servers.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Language identifier for LSP
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum LanguageId {
    Rust,
    Python,
    JavaScript,
    JavaScriptReact,
    TypeScript,
    TypeScriptReact,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Scala,
    Html,
    Css,
    Json,
    Yaml,
    Toml,
    Markdown,
    Xml,
    Sql,
    ShellScript,
    PlainText,
}

impl LanguageId {
    /// Get the LSP language ID string
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::JavaScriptReact => "javascriptreact",
            Self::TypeScript => "typescript",
            Self::TypeScriptReact => "typescriptreact",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Ruby => "ruby",
            Self::Php => "php",
            Self::Swift => "swift",
            Self::Kotlin => "kotlin",
            Self::Scala => "scala",
            Self::Html => "html",
            Self::Css => "css",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Markdown => "markdown",
            Self::Xml => "xml",
            Self::Sql => "sql",
            Self::ShellScript => "shellscript",
            Self::PlainText => "plaintext",
        }
    }

    /// Detect language from a file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::Rust),
            "py" | "pyi" | "pyw" => Some(Self::Python),
            "js" | "mjs" => Some(Self::JavaScript),
            "jsx" => Some(Self::JavaScriptReact),
            "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::TypeScriptReact),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "c" | "h" => Some(Self::C),
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" => Some(Self::Cpp),
            "cs" => Some(Self::CSharp),
            "rb" => Some(Self::Ruby),
            "php" => Some(Self::Php),
            "swift" => Some(Self::Swift),
            "kt" | "kts" => Some(Self::Kotlin),
            "scala" => Some(Self::Scala),
            "html" | "htm" => Some(Self::Html),
            "css" => Some(Self::Css),
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "md" | "markdown" => Some(Self::Markdown),
            "xml" => Some(Self::Xml),
            "sql" => Some(Self::Sql),
            "sh" | "bash" | "zsh" => Some(Self::ShellScript),
            "txt" => Some(Self::PlainText),
            _ => None,
        }
    }

    /// Detect language from file path.
    ///
    /// Returns `PlainText` for files with no extension or unknown extensions.
    pub fn from_path(path: &Path) -> Self {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
            .unwrap_or(Self::PlainText)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_id_as_str() {
        assert_eq!(LanguageId::Rust.as_str(), "rust");
        assert_eq!(LanguageId::TypeScriptReact.as_str(), "typescriptreact");
    }

    #[test]
    fn test_from_extension() {
        assert_eq!(LanguageId::from_extension("rs"), Some(LanguageId::Rust));
        assert_eq!(
            LanguageId::from_extension("tsx"),
            Some(LanguageId::TypeScriptReact)
        );
        assert_eq!(LanguageId::from_extension("xyz"), None);
    }

    #[test]
    fn test_from_path() {
        assert_eq!(LanguageId::from_path(Path::new("foo.rs")), LanguageId::Rust);
        assert_eq!(
            LanguageId::from_path(Path::new("bar.py")),
            LanguageId::Python
        );
        assert_eq!(
            LanguageId::from_path(Path::new("baz.tsx")),
            LanguageId::TypeScriptReact
        );
        assert_eq!(
            LanguageId::from_path(Path::new("unknown.xyz")),
            LanguageId::PlainText
        );
        assert_eq!(
            LanguageId::from_path(Path::new("no_extension")),
            LanguageId::PlainText
        );
    }
}

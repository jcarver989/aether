//! Language metadata for LSP language identification
//!
//! This module provides a single source of truth for mapping between:
//! - `LanguageId` enum variants
//! - LSP language ID strings
//! - File extensions
//! - Type aliases (for grep filtering)

use crate::language_id::LanguageId;

/// Metadata for a supported language
#[derive(Debug, Clone, Copy)]
pub struct LanguageMetadata {
    /// The `LanguageId` enum variant
    pub id: LanguageId,
    /// Primary file extension (e.g., "rs" for Rust)
    /// None for `PlainText` since it has no specific extension
    pub primary_extension: Option<&'static str>,
    /// All accepted aliases including the primary id (e.g., `["rust", "rs"]`)
    pub aliases: &'static [&'static str],
    /// All file extensions for this type (e.g., `["rs"]`)
    pub extensions: &'static [&'static str],
}

/// Static registry of all supported languages
pub static LANGUAGE_METADATA: &[LanguageMetadata] = &[
    LanguageMetadata {
        id: LanguageId::Rust,
        primary_extension: Some("rs"),
        aliases: &["rust", "rs"],
        extensions: &["rs"],
    },
    LanguageMetadata {
        id: LanguageId::Python,
        primary_extension: Some("py"),
        aliases: &["python", "py"],
        extensions: &["py", "pyi", "pyw"],
    },
    LanguageMetadata {
        id: LanguageId::JavaScript,
        primary_extension: Some("js"),
        aliases: &["javascript", "js"],
        extensions: &["js", "mjs"],
    },
    LanguageMetadata {
        id: LanguageId::JavaScriptReact,
        primary_extension: Some("jsx"),
        aliases: &["javascript", "js", "javascriptreact", "jsx"],
        extensions: &["jsx"],
    },
    LanguageMetadata {
        id: LanguageId::TypeScript,
        primary_extension: Some("ts"),
        aliases: &["typescript", "ts"],
        extensions: &["ts"],
    },
    LanguageMetadata {
        id: LanguageId::TypeScriptReact,
        primary_extension: Some("tsx"),
        aliases: &["typescript", "ts", "typescriptreact", "tsx"],
        extensions: &["tsx"],
    },
    LanguageMetadata {
        id: LanguageId::Go,
        primary_extension: Some("go"),
        aliases: &["go"],
        extensions: &["go"],
    },
    LanguageMetadata {
        id: LanguageId::Java,
        primary_extension: Some("java"),
        aliases: &["java"],
        extensions: &["java"],
    },
    LanguageMetadata {
        id: LanguageId::C,
        primary_extension: Some("c"),
        aliases: &["c"],
        extensions: &["c", "h"],
    },
    LanguageMetadata {
        id: LanguageId::Cpp,
        primary_extension: Some("cpp"),
        aliases: &["cpp", "c++"],
        extensions: &["cpp", "cxx", "cc", "hpp", "hxx", "hh"],
    },
    LanguageMetadata {
        id: LanguageId::CSharp,
        primary_extension: Some("cs"),
        aliases: &["csharp", "cs"],
        extensions: &["cs"],
    },
    LanguageMetadata {
        id: LanguageId::Ruby,
        primary_extension: Some("rb"),
        aliases: &["ruby", "rb"],
        extensions: &["rb"],
    },
    LanguageMetadata {
        id: LanguageId::Php,
        primary_extension: Some("php"),
        aliases: &["php"],
        extensions: &["php"],
    },
    LanguageMetadata {
        id: LanguageId::Swift,
        primary_extension: Some("swift"),
        aliases: &["swift"],
        extensions: &["swift"],
    },
    LanguageMetadata {
        id: LanguageId::Kotlin,
        primary_extension: Some("kt"),
        aliases: &["kotlin"],
        extensions: &["kt", "kts"],
    },
    LanguageMetadata {
        id: LanguageId::Scala,
        primary_extension: Some("scala"),
        aliases: &["scala"],
        extensions: &["scala"],
    },
    LanguageMetadata {
        id: LanguageId::Html,
        primary_extension: Some("html"),
        aliases: &["html"],
        extensions: &["html", "htm"],
    },
    LanguageMetadata {
        id: LanguageId::Css,
        primary_extension: Some("css"),
        aliases: &["css"],
        extensions: &["css"],
    },
    LanguageMetadata {
        id: LanguageId::Json,
        primary_extension: Some("json"),
        aliases: &["json"],
        extensions: &["json"],
    },
    LanguageMetadata {
        id: LanguageId::Yaml,
        primary_extension: Some("yaml"),
        aliases: &["yaml", "yml"],
        extensions: &["yaml", "yml"],
    },
    LanguageMetadata {
        id: LanguageId::Toml,
        primary_extension: Some("toml"),
        aliases: &["toml"],
        extensions: &["toml"],
    },
    LanguageMetadata {
        id: LanguageId::Markdown,
        primary_extension: Some("md"),
        aliases: &["markdown", "md"],
        extensions: &["md", "markdown"],
    },
    LanguageMetadata {
        id: LanguageId::Xml,
        primary_extension: Some("xml"),
        aliases: &["xml"],
        extensions: &["xml"],
    },
    LanguageMetadata {
        id: LanguageId::Sql,
        primary_extension: Some("sql"),
        aliases: &["sql"],
        extensions: &["sql"],
    },
    LanguageMetadata {
        id: LanguageId::ShellScript,
        primary_extension: Some("sh"),
        aliases: &["sh", "shell", "bash"],
        extensions: &["sh", "bash", "zsh"],
    },
    LanguageMetadata {
        id: LanguageId::PlainText,
        primary_extension: None,
        aliases: &["plaintext", "text", "txt"],
        extensions: &["txt"],
    },
];

/// Get metadata for a specific `LanguageId`
pub fn metadata_for(id: LanguageId) -> Option<&'static LanguageMetadata> {
    LANGUAGE_METADATA.iter().find(|m| m.id == id)
}

/// Get `LanguageId` from an LSP language ID string
pub fn from_lsp_id(lsp_id: &str) -> Option<LanguageId> {
    LANGUAGE_METADATA
        .iter()
        .find(|m| m.id.as_str() == lsp_id)
        .map(|m| m.id)
}

/// Get all extensions matching a type alias (for grep filtering)
///
/// Returns extensions from all file types that have the given alias.
/// This means "javascript" returns extensions for both JS and JSX files.
/// Case-insensitive lookup.
pub fn extensions_for_alias(alias: &str) -> Vec<&'static str> {
    let lower = alias.to_lowercase();
    LANGUAGE_METADATA
        .iter()
        .filter(|m| m.aliases.iter().any(|a| *a == lower))
        .flat_map(|m| m.extensions.iter().copied())
        .collect()
}

impl LanguageId {
    /// Get the primary file extension for this language.
    ///
    /// Returns None for `PlainText` since it has no specific extension.
    pub fn primary_extension(self) -> Option<&'static str> {
        metadata_for(self).and_then(|m| m.primary_extension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_for() {
        let meta = metadata_for(LanguageId::Rust).unwrap();
        assert_eq!(meta.id.as_str(), "rust");
        assert_eq!(meta.primary_extension, Some("rs"));
        assert!(meta.aliases.contains(&"rust"));
        assert!(meta.aliases.contains(&"rs"));
    }

    #[test]
    fn test_from_lsp_id() {
        assert_eq!(from_lsp_id("rust"), Some(LanguageId::Rust));
        assert_eq!(
            from_lsp_id("typescriptreact"),
            Some(LanguageId::TypeScriptReact)
        );
        assert_eq!(from_lsp_id("unknown"), None);
    }

    #[test]
    fn test_primary_extension() {
        assert_eq!(LanguageId::Rust.primary_extension(), Some("rs"));
        assert_eq!(LanguageId::Python.primary_extension(), Some("py"));
        assert_eq!(LanguageId::PlainText.primary_extension(), None);
    }

    #[test]
    fn test_extensions_for_alias() {
        // JavaScript alias should include both .js and .jsx
        let js_exts = extensions_for_alias("javascript");
        assert!(js_exts.contains(&"js"));
        assert!(js_exts.contains(&"mjs"));
        assert!(js_exts.contains(&"jsx"));

        // TypeScript alias should include both .ts and .tsx
        let ts_exts = extensions_for_alias("typescript");
        assert!(ts_exts.contains(&"ts"));
        assert!(ts_exts.contains(&"tsx"));

        // Shell aliases
        let sh_exts = extensions_for_alias("bash");
        assert!(sh_exts.contains(&"sh"));
        assert!(sh_exts.contains(&"bash"));
        assert!(sh_exts.contains(&"zsh"));
    }

    #[test]
    fn test_all_languages_have_metadata() {
        // Ensure every enum variant has metadata
        let variants = [
            LanguageId::Rust,
            LanguageId::Python,
            LanguageId::JavaScript,
            LanguageId::JavaScriptReact,
            LanguageId::TypeScript,
            LanguageId::TypeScriptReact,
            LanguageId::Go,
            LanguageId::Java,
            LanguageId::C,
            LanguageId::Cpp,
            LanguageId::CSharp,
            LanguageId::Ruby,
            LanguageId::Php,
            LanguageId::Swift,
            LanguageId::Kotlin,
            LanguageId::Scala,
            LanguageId::Html,
            LanguageId::Css,
            LanguageId::Json,
            LanguageId::Yaml,
            LanguageId::Toml,
            LanguageId::Markdown,
            LanguageId::Xml,
            LanguageId::Sql,
            LanguageId::ShellScript,
            LanguageId::PlainText,
        ];

        for variant in variants {
            assert!(
                metadata_for(variant).is_some(),
                "Missing metadata for {:?}",
                variant
            );
        }
    }
}

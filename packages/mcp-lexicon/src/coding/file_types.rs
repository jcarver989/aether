//! File type detection utilities
//!
//! Provides a single source of truth for file type detection used by both
//! grep filtering and LSP language identification.

use std::path::Path;

/// Represents a supported file/language type with its metadata
#[derive(Debug, Clone, Copy)]
pub struct FileTypeInfo {
    /// Primary identifier (e.g., "rust", "python")
    pub id: &'static str,
    /// All accepted aliases including the primary id (e.g., ["rust", "rs"])
    pub aliases: &'static [&'static str],
    /// File extensions for this type (e.g., ["rs"])
    pub extensions: &'static [&'static str],
    /// LSP language identifier string (e.g., "rust")
    pub lsp_id: &'static str,
}

/// Static registry of all supported file types
///
/// Note: JavaScript and TypeScript have separate entries for JSX/TSX variants
/// to support distinct LSP language IDs, but share grep aliases.
pub static FILE_TYPES: &[FileTypeInfo] = &[
    FileTypeInfo {
        id: "rust",
        aliases: &["rust", "rs"],
        extensions: &["rs"],
        lsp_id: "rust",
    },
    FileTypeInfo {
        id: "python",
        aliases: &["python", "py"],
        extensions: &["py", "pyi", "pyw"],
        lsp_id: "python",
    },
    FileTypeInfo {
        id: "javascript",
        aliases: &["javascript", "js"],
        extensions: &["js", "mjs"],
        lsp_id: "javascript",
    },
    FileTypeInfo {
        id: "javascriptreact",
        aliases: &["javascript", "js", "javascriptreact", "jsx"],
        extensions: &["jsx"],
        lsp_id: "javascriptreact",
    },
    FileTypeInfo {
        id: "typescript",
        aliases: &["typescript", "ts"],
        extensions: &["ts"],
        lsp_id: "typescript",
    },
    FileTypeInfo {
        id: "typescriptreact",
        aliases: &["typescript", "ts", "typescriptreact", "tsx"],
        extensions: &["tsx"],
        lsp_id: "typescriptreact",
    },
    FileTypeInfo {
        id: "go",
        aliases: &["go"],
        extensions: &["go"],
        lsp_id: "go",
    },
    FileTypeInfo {
        id: "java",
        aliases: &["java"],
        extensions: &["java"],
        lsp_id: "java",
    },
    FileTypeInfo {
        id: "c",
        aliases: &["c"],
        extensions: &["c", "h"],
        lsp_id: "c",
    },
    FileTypeInfo {
        id: "cpp",
        aliases: &["cpp", "c++"],
        extensions: &["cpp", "cxx", "cc", "hpp", "hxx", "hh"],
        lsp_id: "cpp",
    },
    FileTypeInfo {
        id: "csharp",
        aliases: &["csharp", "cs"],
        extensions: &["cs"],
        lsp_id: "csharp",
    },
    FileTypeInfo {
        id: "ruby",
        aliases: &["ruby", "rb"],
        extensions: &["rb"],
        lsp_id: "ruby",
    },
    FileTypeInfo {
        id: "php",
        aliases: &["php"],
        extensions: &["php"],
        lsp_id: "php",
    },
    FileTypeInfo {
        id: "swift",
        aliases: &["swift"],
        extensions: &["swift"],
        lsp_id: "swift",
    },
    FileTypeInfo {
        id: "kotlin",
        aliases: &["kotlin"],
        extensions: &["kt", "kts"],
        lsp_id: "kotlin",
    },
    FileTypeInfo {
        id: "scala",
        aliases: &["scala"],
        extensions: &["scala"],
        lsp_id: "scala",
    },
    FileTypeInfo {
        id: "html",
        aliases: &["html"],
        extensions: &["html", "htm"],
        lsp_id: "html",
    },
    FileTypeInfo {
        id: "css",
        aliases: &["css"],
        extensions: &["css"],
        lsp_id: "css",
    },
    FileTypeInfo {
        id: "json",
        aliases: &["json"],
        extensions: &["json"],
        lsp_id: "json",
    },
    FileTypeInfo {
        id: "yaml",
        aliases: &["yaml", "yml"],
        extensions: &["yaml", "yml"],
        lsp_id: "yaml",
    },
    FileTypeInfo {
        id: "toml",
        aliases: &["toml"],
        extensions: &["toml"],
        lsp_id: "toml",
    },
    FileTypeInfo {
        id: "markdown",
        aliases: &["markdown", "md"],
        extensions: &["md", "markdown"],
        lsp_id: "markdown",
    },
    FileTypeInfo {
        id: "xml",
        aliases: &["xml"],
        extensions: &["xml"],
        lsp_id: "xml",
    },
    FileTypeInfo {
        id: "sql",
        aliases: &["sql"],
        extensions: &["sql"],
        lsp_id: "sql",
    },
    FileTypeInfo {
        id: "shellscript",
        aliases: &["sh", "shell", "bash"],
        extensions: &["sh", "bash", "zsh"],
        lsp_id: "shellscript",
    },
];

/// Get all file extensions matching a type name (used by grep filtering)
///
/// Returns extensions from all file types that have the given alias.
/// This means "javascript" returns extensions for both JS and JSX files.
///
/// Case-insensitive lookup.
pub fn extensions_for_type(type_name: &str) -> Vec<&'static str> {
    let lower = type_name.to_lowercase();
    FILE_TYPES
        .iter()
        .filter(|ft| ft.aliases.iter().any(|a| *a == lower))
        .flat_map(|ft| ft.extensions.iter().copied())
        .collect()
}

/// Get LSP language ID from file path extension
///
/// Returns "plaintext" for unknown extensions.
pub fn lsp_id_from_extension(ext: &str) -> &'static str {
    FILE_TYPES
        .iter()
        .find(|ft| ft.extensions.contains(&ext))
        .map(|ft| ft.lsp_id)
        .unwrap_or("plaintext")
}

/// Get LSP language ID from file path
///
/// Returns "plaintext" for files with no extension or unknown extensions.
pub fn lsp_id_from_path(path: &Path) -> &'static str {
    path.extension()
        .and_then(|e| e.to_str())
        .map(lsp_id_from_extension)
        .unwrap_or("plaintext")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions_for_type_aliases() {
        // Primary name
        assert!(extensions_for_type("rust").contains(&"rs"));

        // Alias
        assert!(extensions_for_type("rs").contains(&"rs"));

        // Case insensitive
        assert!(extensions_for_type("RUST").contains(&"rs"));
        assert!(extensions_for_type("Rust").contains(&"rs"));
    }

    #[test]
    fn test_extensions_for_unknown_type() {
        assert!(extensions_for_type("unknown").is_empty());
        assert!(extensions_for_type("").is_empty());
    }

    #[test]
    fn test_javascript_includes_jsx() {
        // When filtering by "javascript", should include both .js and .jsx
        let exts = extensions_for_type("javascript");
        assert!(exts.contains(&"js"));
        assert!(exts.contains(&"mjs"));
        assert!(exts.contains(&"jsx"));
    }

    #[test]
    fn test_typescript_includes_tsx() {
        let exts = extensions_for_type("typescript");
        assert!(exts.contains(&"ts"));
        assert!(exts.contains(&"tsx"));
    }

    #[test]
    fn test_lsp_id_from_path() {
        assert_eq!(lsp_id_from_path(Path::new("foo.rs")), "rust");
        assert_eq!(lsp_id_from_path(Path::new("bar.py")), "python");
        assert_eq!(lsp_id_from_path(Path::new("baz.unknown")), "plaintext");
        assert_eq!(lsp_id_from_path(Path::new("no_extension")), "plaintext");
    }

    #[test]
    fn test_jsx_tsx_distinct_lsp_ids() {
        // JS and JSX should have different LSP IDs
        assert_eq!(lsp_id_from_path(Path::new("app.js")), "javascript");
        assert_eq!(lsp_id_from_path(Path::new("app.jsx")), "javascriptreact");

        // TS and TSX should have different LSP IDs
        assert_eq!(lsp_id_from_path(Path::new("app.ts")), "typescript");
        assert_eq!(lsp_id_from_path(Path::new("app.tsx")), "typescriptreact");
    }

    #[test]
    fn test_python_extensions() {
        let exts = extensions_for_type("python");
        assert!(exts.contains(&"py"));
        assert!(exts.contains(&"pyi"));
        assert!(exts.contains(&"pyw"));
    }

    #[test]
    fn test_cpp_extensions() {
        let exts = extensions_for_type("cpp");
        assert!(exts.contains(&"cpp"));
        assert!(exts.contains(&"cxx"));
        assert!(exts.contains(&"cc"));
        assert!(exts.contains(&"hpp"));
        assert!(exts.contains(&"hxx"));
        assert!(exts.contains(&"hh"));
    }

    #[test]
    fn test_shell_aliases() {
        // All shell aliases should work
        assert!(!extensions_for_type("sh").is_empty());
        assert!(!extensions_for_type("shell").is_empty());
        assert!(!extensions_for_type("bash").is_empty());

        // Should all return the same extensions
        let sh_exts = extensions_for_type("sh");
        let shell_exts = extensions_for_type("shell");
        let bash_exts = extensions_for_type("bash");

        assert_eq!(sh_exts, shell_exts);
        assert_eq!(shell_exts, bash_exts);
    }
}

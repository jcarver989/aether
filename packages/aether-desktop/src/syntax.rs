//! Syntax highlighting utilities using syntect.
//!
//! Provides shared syntax highlighting functionality for markdown code blocks
//! and diff views.

use std::io::Cursor;
use std::sync::LazyLock;
use syntect::highlighting::Theme;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Catppuccin Mocha theme embedded at compile time.
const CATPPUCCIN_MOCHA: &[u8] = include_bytes!("../assets/themes/catppuccin-mocha.tmTheme");

/// Loaded syntax definitions - shared across the application.
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

/// The highlighting theme - using Catppuccin Mocha for vibrant colors.
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    syntect::highlighting::ThemeSet::load_from_reader(&mut Cursor::new(CATPPUCCIN_MOCHA))
        .expect("Failed to load Catppuccin Mocha theme")
});

/// Highlights a code block and returns HTML with inline styles.
pub fn highlight_code(code: &str, language: &str) -> String {
    let syntax = find_syntax(language);
    highlighted_html_for_string(code, &SYNTAX_SET, syntax, &THEME)
        .unwrap_or_else(|_| html_escape(code))
}

/// Highlights a single line of code and returns HTML with inline styles.
///
/// This is used for diff views where each line is rendered separately.
pub fn highlight_line(code: &str, language: &str) -> String {
    let syntax = find_syntax(language);

    match highlighted_html_for_string(code, &SYNTAX_SET, syntax, &THEME) {
        Ok(html) => strip_pre_tags(&html),
        Err(_) => html_escape(code),
    }
}

/// Gets the language identifier from a file path's extension.
///
/// Returns the extension if found, or an empty string for unknown file types.
pub fn language_from_path(path: &str) -> &str {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
}

/// Escapes HTML special characters for safe rendering.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Finds syntax definition by language token or file extension.
fn find_syntax(language: &str) -> &'static SyntaxReference {
    SYNTAX_SET
        .find_syntax_by_token(language)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension(language))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text())
}

/// Strips the outer `<pre>` tags from syntect's HTML output.
fn strip_pre_tags(html: &str) -> String {
    // syntect wraps output in <pre style="...">...</pre>
    // We need to extract just the inner content
    let start = html.find('>').map(|i| i + 1).unwrap_or(0);
    let end = html.rfind("</pre>").unwrap_or(html.len());
    html[start..end].trim_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<div>"), "&lt;div&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_language_from_path() {
        assert_eq!(language_from_path("src/main.rs"), "rs");
        assert_eq!(language_from_path("README.md"), "md");
        assert_eq!(language_from_path("Makefile"), "");
        assert_eq!(language_from_path("path/to/file.tsx"), "tsx");
    }

    #[test]
    fn test_highlight_line_rust() {
        let result = highlight_line("let x = 42;", "rs");
        assert!(result.contains("<span"));
        assert!(!result.contains("<pre"));
        assert!(!result.contains("</pre>"));
    }

    #[test]
    fn test_highlight_line_unknown_language() {
        let result = highlight_line("some text", "unknown_lang_xyz");
        // Should still produce output (falls back to plain text)
        assert!(!result.is_empty());
    }

    #[test]
    fn test_highlight_code_preserves_content() {
        let code = "fn main() {}";
        let result = highlight_code(code, "rs");
        // The content should be preserved (might be wrapped in spans)
        assert!(result.contains("main"));
    }

    #[test]
    fn test_find_syntax_by_extension() {
        // "rs" works as extension
        let syntax = find_syntax("rs");
        assert_eq!(syntax.name, "Rust");
    }

    #[test]
    fn test_find_syntax_by_token() {
        // "rust" works as token
        let syntax = find_syntax("rust");
        assert_eq!(syntax.name, "Rust");
    }

    #[test]
    fn test_strip_pre_tags() {
        let html = "<pre style=\"background-color:#2b303b;\">\ncode here\n</pre>";
        assert_eq!(strip_pre_tags(html), "code here");

        // Handles edge cases
        let no_pre = "just content";
        assert_eq!(strip_pre_tags(no_pre), "just content");
    }
}

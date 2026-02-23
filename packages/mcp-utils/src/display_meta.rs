//! Display metadata for tool responses.
//!
//! This module provides types for generating human-readable display metadata
//! that can be sent alongside tool results via the MCP `_meta` field.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Human-readable display metadata for a tool operation.
///
/// Contains a pre-computed `title` (e.g., "Read file") and `value`
/// (e.g., "Cargo.toml, 156 lines") that consumers render directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDisplayMeta {
    pub title: String,
    pub value: String,
}

impl ToolDisplayMeta {
    pub fn new(title: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
        }
    }
}

/// A preview of removed/added lines for an edit operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffPreview {
    pub removed: Vec<String>,
    pub added: Vec<String>,
    pub lang_hint: String,
}

/// Typed wrapper for the MCP `_meta` field on tool results.
///
/// Wraps a [`ToolDisplayMeta`] so that tool output structs can use
/// `Option<ToolResultMeta>` instead of `Option<serde_json::Value>`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResultMeta {
    pub display: ToolDisplayMeta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_preview: Option<DiffPreview>,
}

impl From<ToolDisplayMeta> for ToolResultMeta {
    fn from(display: ToolDisplayMeta) -> Self {
        Self {
            display,
            diff_preview: None,
        }
    }
}

/// Extract a lowercased file extension from a path, for use as a syntax hint.
pub fn extension_hint(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
}

impl ToolResultMeta {
    /// Convert this metadata wrapper into an ACP-compatible meta map.
    pub fn into_map(self) -> serde_json::Map<String, serde_json::Value> {
        match serde_json::to_value(self).expect("ToolResultMeta should serialize") {
            serde_json::Value::Object(map) => map,
            _ => unreachable!("ToolResultMeta should serialize to a JSON object"),
        }
    }

    /// Deserialize metadata wrapper from an ACP-compatible meta map.
    pub fn from_map(map: &serde_json::Map<String, serde_json::Value>) -> Option<Self> {
        serde_json::from_value(serde_json::Value::Object(map.clone())).ok()
    }
}

/// Helper to truncate a string for display purposes.
///
/// Truncates the string to `max_length` characters, adding "..." if truncated.
pub fn truncate(s: &str, max_length: usize) -> String {
    if s.chars().count() <= max_length {
        s.to_string()
    } else {
        let mut truncated = s
            .chars()
            .take(max_length.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

/// Extract the filename from a path, handling both Unix and Windows separators.
pub fn basename(path: &str) -> String {
    let platform_basename = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);

    if platform_basename.contains('\\') {
        path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
    } else {
        platform_basename.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_sets_title_and_value() {
        let meta = ToolDisplayMeta::new("Read file", "Cargo.toml, 156 lines");
        assert_eq!(meta.title, "Read file");
        assert_eq!(meta.value, "Cargo.toml, 156 lines");
    }

    #[test]
    fn test_serde_roundtrip() {
        let meta = ToolDisplayMeta::new("Grep", "'TODO' in src (42 matches)");
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: ToolDisplayMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, meta);
    }

    #[test]
    fn test_serde_json_shape() {
        let meta = ToolDisplayMeta::new("Read file", "Cargo.toml");
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["title"], "Read file");
        assert_eq!(json["value"], "Cargo.toml");
    }

    #[test]
    fn test_tool_result_meta_roundtrip() {
        let meta: ToolResultMeta =
            ToolDisplayMeta::new("Read file", "Cargo.toml, 156 lines").into();
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: ToolResultMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, meta);
    }

    #[test]
    fn test_tool_result_meta_map_roundtrip() {
        let meta: ToolResultMeta =
            ToolDisplayMeta::new("Read file", "Cargo.toml, 156 lines").into();
        let map = meta.clone().into_map();
        let parsed = ToolResultMeta::from_map(&map).expect("should deserialize ToolResultMeta");
        assert_eq!(parsed, meta);
    }

    #[test]
    fn test_tool_result_meta_from_invalid_map_returns_none() {
        let map = serde_json::Map::from_iter([(
            "display".to_string(),
            serde_json::Value::String("not an object".to_string()),
        )]);
        assert!(ToolResultMeta::from_map(&map).is_none());
    }

    #[test]
    fn test_into_result_meta() {
        let display = ToolDisplayMeta::new("Write file", "main.rs");
        let meta: ToolResultMeta = display.clone().into();
        assert_eq!(
            meta,
            ToolResultMeta {
                display,
                diff_preview: None,
            }
        );
    }

    #[test]
    fn test_diff_preview_serde_roundtrip() {
        let preview = DiffPreview {
            removed: vec!["old line".to_string()],
            added: vec!["new line".to_string()],
            lang_hint: "rs".to_string(),
        };
        let json = serde_json::to_string(&preview).unwrap();
        let parsed: DiffPreview = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, preview);
    }

    #[test]
    fn test_tool_result_meta_with_diff_preview() {
        let meta = ToolResultMeta {
            display: ToolDisplayMeta::new("Edit file", "main.rs"),
            diff_preview: Some(DiffPreview {
                removed: vec!["old".to_string()],
                added: vec!["new".to_string()],
                lang_hint: "rs".to_string(),
            }),
        };
        let map = meta.clone().into_map();
        let parsed = ToolResultMeta::from_map(&map).expect("should deserialize");
        assert_eq!(parsed, meta);
    }

    #[test]
    fn test_extension_hint_rs() {
        assert_eq!(extension_hint("/path/to/main.rs"), "rs");
    }

    #[test]
    fn test_extension_hint_uppercase() {
        assert_eq!(extension_hint("README.MD"), "md");
    }

    #[test]
    fn test_extension_hint_no_extension() {
        assert_eq!(extension_hint("Makefile"), "");
    }

    #[test]
    fn test_extension_hint_nested_path() {
        assert_eq!(extension_hint("/foo/bar/baz.tsx"), "tsx");
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("short", 10), "short");
    }

    #[test]
    fn test_truncate_long() {
        let long = "cargo check --message-format=json --locked";
        let truncated = truncate(long, 20);
        assert!(truncated.chars().count() <= 20);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_multibyte() {
        let s = "こんにちは世界テスト文字列"; // 12 chars, each 3 bytes
        let truncated = truncate(s, 8);
        assert!(truncated.chars().count() <= 8);
        assert!(truncated.ends_with("..."));
        assert_eq!(truncated.chars().count(), 8);
    }

    #[test]
    fn test_basename_unix() {
        assert_eq!(basename("/Users/josh/code/aether/Cargo.toml"), "Cargo.toml");
    }

    #[test]
    fn test_basename_windows() {
        assert_eq!(
            basename(r"C:\Users\josh\code\aether\Cargo.toml"),
            "Cargo.toml"
        );
    }

    #[test]
    fn test_basename_bare_name() {
        assert_eq!(basename("Cargo.toml"), "Cargo.toml");
    }
}

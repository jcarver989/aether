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

/// Tag indicating the kind of change a diff line represents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffTag {
    Context,
    Removed,
    Added,
}

/// A single line in a diff, tagged with its change type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffLine {
    pub tag: DiffTag,
    pub content: String,
}

/// A preview of changed lines for an edit operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffPreview {
    pub lines: Vec<DiffLine>,
    pub lang_hint: String,
    /// 1-indexed line number where the edit begins in the original file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
}

/// A snapshot of the agent's current task plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanMeta {
    pub entries: Vec<PlanMetaEntry>,
}

/// A single entry in a plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanMetaEntry {
    pub content: String,
    pub status: PlanMetaStatus,
}

/// Execution status of a plan entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanMetaStatus {
    Pending,
    InProgress,
    Completed,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanMeta>,
}

impl From<ToolDisplayMeta> for ToolResultMeta {
    fn from(display: ToolDisplayMeta) -> Self {
        Self::new(display)
    }
}

impl ToolResultMeta {
    /// Create a new metadata wrapper with just display info.
    pub fn new(display: ToolDisplayMeta) -> Self {
        Self {
            display,
            diff_preview: None,
            plan: None,
        }
    }

    /// Create a metadata wrapper with a plan.
    pub fn with_plan(display: ToolDisplayMeta, plan: PlanMeta) -> Self {
        Self {
            display,
            diff_preview: None,
            plan: Some(plan),
        }
    }

    /// Create a metadata wrapper with a diff preview.
    pub fn with_diff_preview(display: ToolDisplayMeta, diff_preview: DiffPreview) -> Self {
        Self {
            display,
            diff_preview: Some(diff_preview),
            plan: None,
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
                plan: None,
            }
        );
    }

    #[test]
    fn test_diff_preview_serde_roundtrip() {
        let preview = DiffPreview {
            lines: vec![
                DiffLine {
                    tag: DiffTag::Removed,
                    content: "old line".to_string(),
                },
                DiffLine {
                    tag: DiffTag::Added,
                    content: "new line".to_string(),
                },
            ],
            lang_hint: "rs".to_string(),
            start_line: None,
        };
        let json = serde_json::to_string(&preview).unwrap();
        let parsed: DiffPreview = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, preview);
    }

    #[test]
    fn test_tool_result_meta_with_diff_preview() {
        let meta = ToolResultMeta::with_diff_preview(
            ToolDisplayMeta::new("Edit file", "main.rs"),
            DiffPreview {
                lines: vec![
                    DiffLine {
                        tag: DiffTag::Removed,
                        content: "old".to_string(),
                    },
                    DiffLine {
                        tag: DiffTag::Added,
                        content: "new".to_string(),
                    },
                ],
                lang_hint: "rs".to_string(),
                start_line: None,
            },
        );
        let map = meta.clone().into_map();
        let parsed = ToolResultMeta::from_map(&map).expect("should deserialize");
        assert_eq!(parsed, meta);
    }

    #[test]
    fn diff_line_serde_roundtrip() {
        let line = DiffLine {
            tag: DiffTag::Context,
            content: "unchanged".to_string(),
        };
        let json = serde_json::to_string(&line).unwrap();
        let parsed: DiffLine = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, line);
    }

    #[test]
    fn diff_tag_serde_snake_case() {
        assert_eq!(
            serde_json::to_value(DiffTag::Context).unwrap(),
            serde_json::Value::String("context".to_string()),
        );
        assert_eq!(
            serde_json::to_value(DiffTag::Removed).unwrap(),
            serde_json::Value::String("removed".to_string()),
        );
        assert_eq!(
            serde_json::to_value(DiffTag::Added).unwrap(),
            serde_json::Value::String("added".to_string()),
        );
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

    #[test]
    fn test_plan_meta_serde_roundtrip() {
        let plan = PlanMeta {
            entries: vec![
                PlanMetaEntry {
                    content: "Research AI agents".to_string(),
                    status: PlanMetaStatus::Completed,
                },
                PlanMetaEntry {
                    content: "Implement tracking".to_string(),
                    status: PlanMetaStatus::InProgress,
                },
                PlanMetaEntry {
                    content: "Write tests".to_string(),
                    status: PlanMetaStatus::Pending,
                },
            ],
        };
        let json = serde_json::to_string(&plan).unwrap();
        let parsed: PlanMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, plan);
    }

    #[test]
    fn test_plan_meta_status_serde_snake_case() {
        let json = serde_json::to_value(&PlanMetaStatus::InProgress).unwrap();
        assert_eq!(json, serde_json::Value::String("in_progress".to_string()));
    }

    #[test]
    fn test_tool_result_meta_with_plan() {
        let meta = ToolResultMeta::with_plan(
            ToolDisplayMeta::new("Todo", "Research AI agents"),
            PlanMeta {
                entries: vec![PlanMetaEntry {
                    content: "Research AI agents".to_string(),
                    status: PlanMetaStatus::InProgress,
                }],
            },
        );
        let map = meta.clone().into_map();
        let parsed = ToolResultMeta::from_map(&map).expect("should deserialize");
        assert_eq!(parsed, meta);
    }

    #[test]
    fn test_tool_result_meta_plan_omitted_when_none() {
        let meta: ToolResultMeta = ToolDisplayMeta::new("Read file", "main.rs").into();
        let json = serde_json::to_value(&meta).unwrap();
        assert!(json.get("plan").is_none());
    }

    #[test]
    fn test_diff_preview_start_line_roundtrip() {
        let preview = DiffPreview {
            lines: vec![
                DiffLine {
                    tag: DiffTag::Removed,
                    content: "old".to_string(),
                },
                DiffLine {
                    tag: DiffTag::Added,
                    content: "new".to_string(),
                },
            ],
            lang_hint: "rs".to_string(),
            start_line: Some(42),
        };
        let json = serde_json::to_string(&preview).unwrap();
        let parsed: DiffPreview = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.start_line, Some(42));
    }

    #[test]
    fn test_diff_preview_start_line_omitted_when_none() {
        let preview = DiffPreview {
            lines: vec![],
            lang_hint: String::new(),
            start_line: None,
        };
        let json = serde_json::to_value(&preview).unwrap();
        assert!(json.get("start_line").is_none());
    }

    #[test]
    fn test_diff_preview_missing_start_line_defaults_to_none() {
        let json = r#"{"lines":[],"lang_hint":"rs"}"#;
        let parsed: DiffPreview = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.start_line, None);
    }
}

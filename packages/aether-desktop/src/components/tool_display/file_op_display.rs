//! File operation display components for read/write/edit operations.

use dioxus::prelude::*;

use crate::components::tool_display::types::{
    EditFileDisplayMeta, ReadFileDisplayMeta, WriteFileDisplayMeta,
};

/// Display a read_file tool result.
#[component]
pub fn ReadFileDisplay(read_meta: ReadFileDisplayMeta) -> Element {
    let size_str = read_meta.size.map(format_size);
    let lines_str = read_meta.lines.map(|lines| format!("{} lines", lines));

    rsx! {
        div {
            class: "flex flex-col gap-1 py-1",

            div {
                class: "flex items-center gap-2",
                span { class: "text-blue-400 text-xs", "📄" }
                span { class: "font-mono text-sm text-gray-200", "{read_meta.file_path}" }
            }

            div {
                class: "flex items-center gap-4 ml-5 text-xs text-gray-500",
                if let Some(size) = &size_str {
                    span { "{size}" }
                }
                if let Some(lines) = &lines_str {
                    span { "{lines}" }
                }
            }
        }
    }
}

/// Display a write_file tool result.
#[component]
pub fn WriteFileDisplay(write_meta: WriteFileDisplayMeta) -> Element {
    let formatted = write_meta.size.map(format_size);

    rsx! {
        div {
            class: "flex flex-col gap-1 py-1",

            div {
                class: "flex items-center gap-2",
                span { class: "text-green-400 text-xs", "✎" }
                span { class: "font-mono text-sm text-gray-200", "{write_meta.file_path}" }
            }

            if let Some(size_str) = &formatted {
                div {
                    class: "flex items-center gap-2 ml-5",
                    span { class: "text-xs text-gray-500", "{size_str}" }
                }
            }
        }
    }
}

/// Display an edit_file tool result with abbreviated diff.
#[component]
pub fn EditFileDisplay(edit_meta: EditFileDisplayMeta) -> Element {
    let truncated_old = edit_meta.old_text.as_ref().map(|t| truncate_text(t, 60));
    let truncated_new = edit_meta.new_text.as_ref().map(|t| truncate_text(t, 60));
    let show_diff = truncated_old.is_some() || truncated_new.is_some();

    rsx! {
        div {
            class: "flex flex-col gap-1 py-1",

            div {
                class: "flex items-center gap-2",
                span { class: "text-yellow-400 text-xs", "✎" }
                span { class: "font-mono text-sm text-gray-200", "{edit_meta.file_path}" }
            }

            if show_diff {
                div {
                    class: "ml-5 mt-1 font-mono text-xs bg-black/30 rounded px-2 py-1",

                    if let Some(old) = &truncated_old {
                        div {
                            class: "text-red-400 line-through opacity-70",
                            "- {old}"
                        }
                    }

                    if let Some(new) = &truncated_new {
                        div {
                            class: "text-green-400",
                            "+ {new}"
                        }
                    }
                }
            }
        }
    }
}

/// Format file size in human-readable format.
fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes / 1024)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{} MB", bytes / (1024 * 1024))
    } else {
        format!("{} GB", bytes / (1024 * 1024 * 1024))
    }
}

/// Truncate text to a maximum length with ellipsis.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        let mut truncated = text.chars().take(max_len - 3).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(500), "500 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(2048), "2 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(2 * 1024 * 1024), "2 MB");
    }

    #[test]
    fn test_truncate_text_short() {
        assert_eq!(truncate_text("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_text_long() {
        let result = truncate_text("this is a very long text that should be truncated", 20);
        assert!(result.len() <= 20);
        assert!(result.ends_with("..."));
    }
}

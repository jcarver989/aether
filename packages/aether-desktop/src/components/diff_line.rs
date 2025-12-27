//! Diff line component for rendering individual diff lines.

use dioxus::prelude::*;

use crate::state::{DiffLine as DiffLineData, LineOrigin};

/// Renders a single line in a diff with appropriate styling.
#[component]
pub fn DiffLineRow(line: DiffLineData, show_line_numbers: bool) -> Element {
    let (bg_class, text_class, gutter_class, origin_char) = match line.origin {
        LineOrigin::Addition => (
            "bg-green-500/10",
            "text-green-400",
            "bg-green-500/20 text-green-400",
            "+",
        ),
        LineOrigin::Deletion => (
            "bg-red-500/10",
            "text-red-400",
            "bg-red-500/20 text-red-400",
            "-",
        ),
        LineOrigin::Context => ("", "text-gray-300", "text-gray-500", " "),
    };

    // Format line numbers - show "-" for missing numbers
    let old_lineno = line
        .old_lineno
        .map(|n| format!("{:>4}", n))
        .unwrap_or_else(|| "    ".to_string());
    let new_lineno = line
        .new_lineno
        .map(|n| format!("{:>4}", n))
        .unwrap_or_else(|| "    ".to_string());

    // Remove trailing newline for display
    let content = line.content.trim_end_matches('\n');

    rsx! {
        div {
            class: "flex font-mono text-sm leading-relaxed {bg_class}",

            if show_line_numbers {
                // Old line number gutter
                span {
                    class: "w-12 px-2 text-right text-gray-500 select-none border-r border-[#2d313a] flex-shrink-0",
                    "{old_lineno}"
                }
                // New line number gutter
                span {
                    class: "w-12 px-2 text-right text-gray-500 select-none border-r border-[#2d313a] flex-shrink-0",
                    "{new_lineno}"
                }
            }

            // Origin indicator (+, -, or space)
            span {
                class: "w-6 text-center select-none flex-shrink-0 {gutter_class}",
                "{origin_char}"
            }

            // Line content
            pre {
                class: "flex-1 px-2 overflow-x-auto whitespace-pre {text_class}",
                "{content}"
            }
        }
    }
}

/// Renders a hunk header line (e.g., @@ -1,5 +1,7 @@).
#[component]
pub fn HunkHeader(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> Element {
    rsx! {
        div {
            class: "flex font-mono text-sm bg-blue-500/10 text-blue-400 py-1",
            span {
                class: "px-4",
                "@@ -{old_start},{old_lines} +{new_start},{new_lines} @@"
            }
        }
    }
}

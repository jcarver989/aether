//! Diff line component for rendering individual diff lines.

use dioxus::prelude::*;

use crate::state::{DiffLine as DiffLineData, LineOrigin};
use crate::syntax::highlight_line;

/// Renders a single line in a diff with appropriate styling.
#[component]
pub fn DiffLineRow(line: DiffLineData, show_line_numbers: bool, language: String) -> Element {
    // Use solid dark colors instead of transparency for better font rendering
    let (bg_class, gutter_class, origin_char) = match line.origin {
        LineOrigin::Addition => ("bg-[#1a2e1a]", "bg-[#243d24] text-green-400", "+"),
        LineOrigin::Deletion => ("bg-[#2e1a1a]", "bg-[#3d2424] text-red-400", "-"),
        LineOrigin::Context => ("", "text-gray-500", " "),
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

    // Remove trailing newline for display and apply syntax highlighting
    let content = line.content.trim_end_matches('\n');
    let highlighted_content = highlight_line(content, &language);

    rsx! {
        div {
            class: "flex text-sm leading-relaxed {bg_class}",
            style: "font-family: var(--font-family-mono); font-weight: 400",

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

            // Line content with syntax highlighting
            pre {
                class: "flex-1 px-2 overflow-x-auto whitespace-pre",
                dangerous_inner_html: "{highlighted_content}",
            }
        }
    }
}

/// Renders a hunk header line (e.g., @@ -1,5 +1,7 @@).
#[component]
pub fn HunkHeader(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> Element {
    rsx! {
        div {
            class: "flex text-sm bg-blue-500/10 text-blue-400 py-1",
            style: "font-family: var(--font-family-mono); font-weight: 400",
            span {
                class: "px-4",
                "@@ -{old_start},{old_lines} +{new_start},{new_lines} @@"
            }
        }
    }
}

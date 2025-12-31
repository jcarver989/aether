//! Diff line component for rendering individual diff lines.

use dioxus::prelude::*;

use crate::state::{DiffComment, DiffLine as DiffLineData, LineOrigin};
use crate::syntax::highlight_line;

use super::diff_comment::{CommentBubble, CommentInput, CommentMarker, LineInfo};

/// Renders a single line in a diff with appropriate styling and comment support.
#[component]
pub fn DiffLineRow(
    line: DiffLineData,
    show_line_numbers: bool,
    language: String,
    file_path: String,
    comment: Option<DiffComment>,
    show_comment_input: bool,
    on_comment_click: Option<EventHandler<LineInfo>>,
    on_comment_save: Option<EventHandler<String>>,
    on_comment_cancel: Option<EventHandler<()>>,
    on_comment_edit: Option<EventHandler<String>>,
    on_comment_delete: Option<EventHandler<()>>,
) -> Element {
    let mut is_hovered = use_signal(|| false);

    // Use solid dark colors instead of transparency for better font rendering
    let (bg_class, gutter_class, origin_char) = match line.origin {
        LineOrigin::Addition => ("bg-[#1a2e1a]", "bg-[#243d24] text-green-400", "+"),
        LineOrigin::Deletion => ("bg-[#2e1a1a]", "bg-[#3d2424] text-red-400", "-"),
        LineOrigin::Context => ("", "text-gray-500", " "),
    };

    // Determine the line number to use for comments
    // For additions, use new_lineno; for deletions, use old_lineno
    let comment_line_number = match line.origin {
        LineOrigin::Addition | LineOrigin::Context => line.new_lineno,
        LineOrigin::Deletion => line.old_lineno,
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
    let content = line.content.trim_end_matches('\n').to_string();
    let highlighted_content = highlight_line(&content, &language);

    // Can this line be commented on?
    let can_comment = on_comment_click.is_some() && comment_line_number.is_some();

    // Hover class for commentable lines
    let hover_class = if is_hovered() && can_comment {
        "ring-1 ring-inset ring-green-500/30"
    } else {
        ""
    };

    let line_info = LineInfo {
        file_path: file_path.clone(),
        line_number: comment_line_number.unwrap_or(0),
        line_origin: line.origin,
        content: content.clone(),
    };

    rsx! {
        div {
            // pl-8 creates space for the comment marker button inside the bounding box
            class: "group relative pl-8",
            onmouseenter: move |_| is_hovered.set(true),
            onmouseleave: move |_| is_hovered.set(false),

            // Comment marker (shown on hover, positioned in the left padding)
            if is_hovered() && can_comment && !show_comment_input && comment.is_none() {
                div {
                    class: "absolute left-2 top-1/2 -translate-y-1/2 z-10",
                    CommentMarker {
                        on_click: {
                            let line_info = line_info.clone();
                            move |_| {
                                if let Some(handler) = &on_comment_click {
                                    handler.call(line_info.clone());
                                }
                            }
                        },
                    }
                }
            }

            // Main line row
            div {
                class: "flex text-sm leading-relaxed {bg_class} {hover_class}",
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
                    class: "flex-1 px-2 whitespace-pre",
                    dangerous_inner_html: "{highlighted_content}",
                }
            }

            // Comment input (when adding a new comment)
            if show_comment_input {
                CommentInput {
                    initial_value: None,
                    on_save: move |content| {
                        if let Some(handler) = &on_comment_save {
                            handler.call(content);
                        }
                    },
                    on_cancel: move |_| {
                        if let Some(handler) = &on_comment_cancel {
                            handler.call(());
                        }
                    },
                }
            }

            // Existing comment bubble
            if let Some(existing_comment) = comment {
                CommentBubble {
                    comment: existing_comment,
                    on_edit: move |new_content| {
                        if let Some(handler) = &on_comment_edit {
                            handler.call(new_content);
                        }
                    },
                    on_delete: move |_| {
                        if let Some(handler) = &on_comment_delete {
                            handler.call(());
                        }
                    },
                }
            }
        }
    }
}

/// Renders a hunk header line (e.g., @@ -1,5 +1,7 @@).
#[component]
pub fn HunkHeader(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> Element {
    rsx! {
        div {
            class: "flex text-sm bg-green-500/10 text-green-400 py-1",
            style: "font-family: var(--font-family-mono); font-weight: 400",
            span {
                class: "px-4",
                "@@ -{old_start},{old_lines} +{new_start},{new_lines} @@"
            }
        }
    }
}

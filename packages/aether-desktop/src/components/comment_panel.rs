//! Comment panel component for displaying all pending comments.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::state::{generate_comments_prompt, CommentKey, DiffComment, LineOrigin};

/// Panel showing all pending comments with Send to Agent functionality.
#[component]
pub fn CommentPanel(
    comments: HashMap<CommentKey, DiffComment>,
    on_send: EventHandler<String>,
    on_clear: EventHandler<()>,
    on_remove_comment: EventHandler<CommentKey>,
) -> Element {
    let mut show_preview = use_signal(|| false);
    let mut confirm_clear = use_signal(|| false);

    if comments.is_empty() {
        return rsx! { EmptyState {} };
    }

    let by_file: HashMap<&str, Vec<&DiffComment>> =
        comments.values().fold(HashMap::new(), |mut acc, comment| {
            acc.entry(&comment.file_path).or_default().push(comment);
            acc
        });

    // Sort files for consistent ordering
    let mut files: Vec<_> = by_file.keys().cloned().collect();
    files.sort();

    // Generate the prompt for preview
    let prompt = generate_comments_prompt(&comments);

    rsx! {
        div {
            class: "h-full flex flex-col bg-[#12151a]",

            // Header
            div {
                class: "p-3 border-b border-[#2d313a] flex items-center justify-between",
                h3 {
                    class: "text-sm font-medium text-gray-200",
                    "Comments ({comments.len()})"
                }
                button {
                    class: "text-xs text-gray-500 hover:text-gray-300 transition-colors",
                    onclick: move |_| show_preview.set(!show_preview()),
                    if show_preview() { "Hide Preview" } else { "Show Preview" }
                }
            }

            // Preview section (collapsible)
            if show_preview() {
                div {
                    class: "p-3 bg-[#0f1116] border-b border-[#2d313a] max-h-48 overflow-y-auto",
                    pre {
                        class: "text-xs text-gray-400 whitespace-pre-wrap font-mono",
                        "{prompt}"
                    }
                }
            }

            // Comments list
            div {
                class: "flex-1 overflow-y-auto p-3 space-y-4",

                for file_path in files.iter() {
                    {
                        let mut file_comments = by_file.get(*file_path).cloned().unwrap_or_default();
                        file_comments.sort_by_key(|c| c.line_number);

                        rsx! {
                            div {
                                key: "{file_path}",
                                class: "space-y-2",

                                // File header
                                h4 {
                                    class: "text-xs font-mono text-gray-400 truncate",
                                    title: "{file_path}",
                                    "{file_path}"
                                }

                                // File's comments
                                for comment in file_comments.iter() {
                                    CommentItem {
                                        key: "{comment.file_path}-{comment.line_number}",
                                        comment: (*comment).clone(),
                                        on_remove: move |key| on_remove_comment.call(key),
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Action buttons
            div {
                class: "p-3 border-t border-[#2d313a] space-y-2",

                // Send to Agent button
                button {
                    class: "w-full py-2 px-4 bg-blue-600 text-white rounded-lg hover:bg-blue-500 transition-colors font-medium text-sm",
                    onclick: move |_| {
                        on_send.call(prompt.clone());
                    },
                    "Send to Agent"
                }

                // Clear all button with confirmation
                if confirm_clear() {
                    div {
                        class: "flex gap-2",
                        button {
                            class: "flex-1 py-1.5 px-3 bg-red-600 text-white rounded text-sm hover:bg-red-500 transition-colors",
                            onclick: move |_| {
                                on_clear.call(());
                                confirm_clear.set(false);
                            },
                            "Confirm Clear"
                        }
                        button {
                            class: "flex-1 py-1.5 px-3 bg-gray-700 text-gray-200 rounded text-sm hover:bg-gray-600 transition-colors",
                            onclick: move |_| confirm_clear.set(false),
                            "Cancel"
                        }
                    }
                } else {
                    button {
                        class: "w-full py-1.5 px-3 text-gray-400 hover:text-gray-200 text-sm transition-colors",
                        onclick: move |_| confirm_clear.set(true),
                        "Clear All"
                    }
                }
            }
        }
    }
}

/// Empty state component displayed when there are no comments.
#[component]
fn EmptyState() -> Element {
    rsx! {
        div {
            class: "h-full flex flex-col items-center justify-center p-6 text-gray-500",
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "48",
                height: "48",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.5",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                class: "mb-4 text-gray-600",
                path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
            }
            p { class: "text-sm", "No comments yet" }
            p { class: "text-xs text-gray-600 mt-1", "Click on diff lines to add comments" }
        }
    }
}

/// A single comment item in the panel.
#[component]
fn CommentItem(comment: DiffComment, on_remove: EventHandler<CommentKey>) -> Element {
    let origin_class = match comment.line_origin {
        LineOrigin::Addition => "text-green-400",
        LineOrigin::Deletion => "text-red-400",
        LineOrigin::Context => "text-gray-400",
    };

    let key = (comment.file_path.clone(), comment.line_number);

    rsx! {
        div {
            class: "bg-[#1a1d23] rounded-lg p-2 group",

            // Line reference
            div {
                class: "flex items-center justify-between text-xs mb-1",
                span {
                    class: "font-mono {origin_class}",
                    "Line {comment.line_number}"
                }
                button {
                    class: "opacity-0 group-hover:opacity-100 text-gray-500 hover:text-red-400 transition-all",
                    onclick: move |_| on_remove.call(key.clone()),
                    "×"
                }
            }

            // Comment content
            p {
                class: "text-sm text-gray-200 line-clamp-2",
                "{comment.content}"
            }
        }
    }
}

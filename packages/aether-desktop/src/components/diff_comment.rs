//! Components for commenting on diff lines.

use dioxus::prelude::*;

use super::prompt_input::PromptInput;
use crate::state::{DiffComment, LineOrigin};

/// Information about a line that can be commented on.
#[derive(Clone, PartialEq, Debug)]
pub struct LineInfo {
    pub file_path: String,
    pub line_number: u32,
    pub line_origin: LineOrigin,
    pub content: String,
}

/// A small "+" button shown on hover to add a comment.
#[component]
pub fn CommentMarker(on_click: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: "comment-marker w-5 h-5 rounded-full bg-green-500 text-black flex items-center justify-center text-xs font-bold hover:bg-green-400 transition-colors",
            onclick: move |e| {
                e.stop_propagation();
                on_click.call(());
            },
            "+"
        }
    }
}

/// Input component for writing a new comment.
#[component]
pub fn CommentInput(
    initial_value: Option<String>,
    on_save: EventHandler<String>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut content = use_signal(|| initial_value.unwrap_or_default());

    let try_save = move || {
        let text = content.read().trim().to_string();
        if !text.is_empty() {
            on_save.call(text);
        }
    };

    let handle_keydown = {
        move |e: KeyboardEvent| match e.key() {
            Key::Enter if e.modifiers().meta() || e.modifiers().ctrl() => {
                e.prevent_default();
                try_save();
            }
            Key::Escape => {
                e.prevent_default();
                on_cancel.call(());
            }
            _ => {}
        }
    };

    rsx! {
        div {
            class: "comment-input bg-[#1a1d23] border border-[#3d4450] rounded-lg p-3 mt-2 ml-8",
            onclick: move |e| e.stop_propagation(),
            onkeydown: handle_keydown,

            PromptInput {
                value: content,
                on_change: move |value: String| {
                    content.set(value);
                },
                on_submit: move |_| try_save(),
                placeholder: "Add a comment...".to_string(),
                disabled: false,
                rows: "3",
                simple: true,
            }

            div {
                class: "flex justify-end gap-2 mt-2",

                button {
                    class: "px-3 py-1 text-sm text-gray-400 hover:text-gray-200 transition-colors",
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }

                button {
                    class: "px-3 py-1 text-sm bg-green-600 text-black rounded hover:bg-green-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed",
                    disabled: content.read().trim().is_empty(),
                    onclick: move |_| try_save(),
                    "Save (Ctrl+Enter)"
                }
            }
        }
    }
}

/// Displays an existing comment with edit/delete options.
#[component]
pub fn CommentBubble(
    comment: DiffComment,
    on_edit: EventHandler<String>,
    on_delete: EventHandler<()>,
) -> Element {
    let mut is_editing = use_signal(|| false);

    if is_editing() {
        let comment_content = comment.content.clone();
        return rsx! {
            CommentInput {
                initial_value: Some(comment_content),
                on_save: move |new_content| {
                    on_edit.call(new_content);
                    is_editing.set(false);
                },
                on_cancel: move |_| is_editing.set(false),
            }
        };
    }

    rsx! {
        div {
            class: "comment-bubble bg-[#1a1d23] border-l-2 border-green-500 rounded-r-lg p-3 mt-2 ml-8 group",

            // Comment content
            p {
                class: "text-sm text-gray-200 whitespace-pre-wrap",
                "{comment.content}"
            }

            // Actions (shown on hover)
            div {
                class: "flex justify-end gap-2 mt-2 opacity-0 group-hover:opacity-100 transition-opacity",

                button {
                    class: "text-xs text-gray-500 hover:text-gray-300 transition-colors",
                    onclick: move |_| is_editing.set(true),
                    "Edit"
                }

                button {
                    class: "text-xs text-red-500 hover:text-red-400 transition-colors",
                    onclick: move |_| on_delete.call(()),
                    "Delete"
                }
            }
        }
    }
}

//! Diff view component for displaying git diffs.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::state::{CommentKey, DiffComment, DiffState, FileDiff, FileStatus, LineOrigin};
use crate::syntax::language_from_path;

use super::comment_panel::CommentPanel;
use super::diff_comment::LineInfo;
use super::diff_line::{DiffLineRow, HunkHeader};
use super::file_drawer::FileDrawer;

/// Main diff view component with file drawer and diff content.
#[component]
pub fn DiffView(
    diff_state: DiffState,
    on_file_select: EventHandler<String>,
    on_add_comment: EventHandler<DiffComment>,
    on_edit_comment: EventHandler<(CommentKey, String)>,
    on_remove_comment: EventHandler<CommentKey>,
    on_clear_comments: EventHandler<()>,
    on_send_comments: EventHandler<String>,
) -> Element {
    // Track which line has an active comment input
    let mut active_comment_input: Signal<Option<CommentKey>> = use_signal(|| None);

    // Find the currently selected file's diff
    let selected_diff = diff_state
        .selected_file
        .as_ref()
        .and_then(|path| diff_state.files.iter().find(|f| &f.path == path));

    let comments_for_panel = diff_state.comments.clone();

    rsx! {
        div {
            class: "flex h-full bg-[#0f1116] overflow-hidden",

            // File drawer (left panel)
            FileDrawer {
                files: diff_state.files.clone(),
                selected: diff_state.selected_file.clone(),
                on_select: move |path| on_file_select.call(path),
            }

            // Diff content (center panel)
            div {
                class: "flex-1 flex flex-col min-w-0 overflow-hidden",

                // Content area
                if diff_state.loading {
                    LoadingState {}
                } else if let Some(error) = &diff_state.error {
                    ErrorState { message: error.clone() }
                } else if diff_state.files.is_empty() {
                    EmptyState {}
                } else if let Some(file_diff) = selected_diff {
                    FileDiffContent {
                        file: file_diff.clone(),
                        comments: diff_state.comments.clone(),
                        active_comment_input: active_comment_input,
                        on_comment_click: move |line_info: LineInfo| {
                            let key = (line_info.file_path.clone(), line_info.line_number);
                            active_comment_input.set(Some(key));
                        },
                        on_comment_save: move |(line_info, content): (LineInfo, String)| {
                            let comment = DiffComment::new(
                                line_info.file_path,
                                line_info.line_number,
                                line_info.line_origin,
                                content,
                                line_info.content,
                            );
                            on_add_comment.call(comment);
                            active_comment_input.set(None);
                        },
                        on_comment_cancel: move |_| {
                            active_comment_input.set(None);
                        },
                        on_comment_edit: move |(key, content): (CommentKey, String)| {
                            on_edit_comment.call((key, content));
                        },
                        on_comment_delete: move |key: CommentKey| {
                            on_remove_comment.call(key);
                        },
                    }
                } else {
                    NoFileSelectedState {}
                }
            }

            // Comment panel (right panel)
            div {
                class: "w-72 border-l border-[#2d313a] flex-shrink-0",
                CommentPanel {
                    comments: comments_for_panel,
                    on_send: move |prompt| on_send_comments.call(prompt),
                    on_clear: move |_| on_clear_comments.call(()),
                    on_remove_comment: move |key| on_remove_comment.call(key),
                }
            }
        }
    }
}

/// Displays the diff content for a single file.
#[component]
fn FileDiffContent(
    file: FileDiff,
    comments: HashMap<CommentKey, DiffComment>,
    active_comment_input: Signal<Option<CommentKey>>,
    on_comment_click: EventHandler<LineInfo>,
    on_comment_save: EventHandler<(LineInfo, String)>,
    on_comment_cancel: EventHandler<()>,
    on_comment_edit: EventHandler<(CommentKey, String)>,
    on_comment_delete: EventHandler<CommentKey>,
) -> Element {
    let (status_text, status_class) = match file.status {
        FileStatus::Added => ("Added", "text-green-400 bg-green-500/10"),
        FileStatus::Modified => ("Modified", "text-blue-400 bg-blue-500/10"),
        FileStatus::Deleted => ("Deleted", "text-red-400 bg-red-500/10"),
        FileStatus::Renamed => ("Renamed", "text-purple-400 bg-purple-500/10"),
    };

    // Derive language from file extension for syntax highlighting
    let language = language_from_path(&file.path).to_string();
    let file_path = file.path.clone();

    rsx! {
        // File header
        div {
            class: "p-3 border-b border-[#2d313a] flex items-center gap-3",
            span {
                class: "px-2 py-0.5 rounded text-xs font-medium {status_class}",
                "{status_text}"
            }
            span {
                class: "font-mono text-sm text-gray-200",
                "{file.path}"
            }
            if let Some(old_path) = &file.old_path {
                span {
                    class: "text-gray-500 text-sm",
                    "(from {old_path})"
                }
            }
        }

        // Diff content
        div {
            class: "flex-1 overflow-auto",

            if file.hunks.is_empty() {
                div {
                    class: "p-4 text-gray-500 text-sm",
                    match file.status {
                        FileStatus::Added => "New file (binary or empty)",
                        FileStatus::Deleted => "File deleted",
                        _ => "No changes to display",
                    }
                }
            }

            for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
                div {
                    key: "{hunk_idx}",

                    // Hunk header
                    HunkHeader {
                        old_start: hunk.old_start,
                        old_lines: hunk.old_lines,
                        new_start: hunk.new_start,
                        new_lines: hunk.new_lines,
                    }

                    // Hunk lines
                    for (line_idx, line) in hunk.lines.iter().enumerate() {
                        {
                            // Determine the line number for this line
                            let line_number = match line.origin {
                                LineOrigin::Addition | LineOrigin::Context => line.new_lineno,
                                LineOrigin::Deletion => line.old_lineno,
                            };

                            let key: Option<CommentKey> = line_number.map(|n| (file_path.clone(), n));
                            let comment = key.as_ref().and_then(|k| comments.get(k).cloned());
                            let show_input = key.as_ref().map(|k| active_comment_input() == Some(k.clone())).unwrap_or(false);

                            let line_info = LineInfo {
                                file_path: file_path.clone(),
                                line_number: line_number.unwrap_or(0),
                                line_origin: line.origin,
                                content: line.content.clone(),
                            };
                            let line_info_for_save = line_info.clone();
                            let key_for_edit = key.clone();
                            let key_for_delete = key.clone();

                            rsx! {
                                DiffLineRow {
                                    key: "{hunk_idx}-{line_idx}",
                                    line: line.clone(),
                                    show_line_numbers: true,
                                    language: language.clone(),
                                    file_path: file_path.clone(),
                                    comment: comment,
                                    show_comment_input: show_input,
                                    on_comment_click: move |info: LineInfo| {
                                        on_comment_click.call(info);
                                    },
                                    on_comment_save: move |content: String| {
                                        on_comment_save.call((line_info_for_save.clone(), content));
                                    },
                                    on_comment_cancel: move |_| {
                                        on_comment_cancel.call(());
                                    },
                                    on_comment_edit: move |new_content: String| {
                                        if let Some(k) = key_for_edit.clone() {
                                            on_comment_edit.call((k, new_content));
                                        }
                                    },
                                    on_comment_delete: move |_| {
                                        if let Some(k) = key_for_delete.clone() {
                                            on_comment_delete.call(k);
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Loading state display.
#[component]
fn LoadingState() -> Element {
    rsx! {
        div {
            class: "flex-1 flex items-center justify-center",
            div {
                class: "flex flex-col items-center gap-3",
                // Simple spinner
                div {
                    class: "w-8 h-8 border-2 border-blue-500 border-t-transparent rounded-full animate-spin",
                }
                span {
                    class: "text-gray-400 text-sm",
                    "Computing diff..."
                }
            }
        }
    }
}

/// Error state display.
#[component]
fn ErrorState(message: String) -> Element {
    rsx! {
        div {
            class: "flex-1 flex items-center justify-center",
            div {
                class: "text-center p-6",
                div {
                    class: "w-12 h-12 mx-auto mb-4 rounded-full bg-red-500/10 flex items-center justify-center",
                    span {
                        class: "text-red-400 text-xl",
                        "!"
                    }
                }
                p {
                    class: "text-red-400 font-medium mb-2",
                    "Failed to compute diff"
                }
                p {
                    class: "text-gray-500 text-sm max-w-xs",
                    "{message}"
                }
            }
        }
    }
}

/// Empty state when no changes detected.
#[component]
fn EmptyState() -> Element {
    rsx! {
        div {
            class: "flex-1 flex items-center justify-center",
            div {
                class: "text-center p-6",
                div {
                    class: "w-16 h-16 mx-auto mb-4 rounded-full bg-gray-700/50 flex items-center justify-center",
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "32",
                        height: "32",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        class: "text-gray-500",
                        path {
                            d: "M9 11l3 3L22 4"
                        }
                        path {
                            d: "M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"
                        }
                    }
                }
                p {
                    class: "text-gray-400 font-medium mb-2",
                    "No changes detected"
                }
                p {
                    class: "text-gray-500 text-sm",
                    "The working directory matches HEAD"
                }
            }
        }
    }
}

/// State when no file is selected.
#[component]
fn NoFileSelectedState() -> Element {
    rsx! {
        div {
            class: "flex-1 flex items-center justify-center",
            div {
                class: "text-center p-6",
                p {
                    class: "text-gray-400",
                    "Select a file to view its diff"
                }
            }
        }
    }
}

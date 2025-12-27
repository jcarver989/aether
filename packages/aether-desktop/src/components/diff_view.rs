//! Diff view component for displaying git diffs.

use dioxus::prelude::*;

use crate::state::{DiffState, FileDiff, FileStatus};
use crate::syntax::language_from_path;

use super::diff_line::{DiffLineRow, HunkHeader};
use super::file_drawer::FileDrawer;

/// Main diff view component with file drawer and diff content.
#[component]
pub fn DiffView(diff_state: DiffState, on_file_select: EventHandler<String>) -> Element {
    // Find the currently selected file's diff
    let selected_diff = diff_state
        .selected_file
        .as_ref()
        .and_then(|path| diff_state.files.iter().find(|f| &f.path == path));

    rsx! {
        div {
            class: "flex h-full bg-[#0f1116] overflow-hidden",

            // File drawer (left panel)
            FileDrawer {
                files: diff_state.files.clone(),
                selected: diff_state.selected_file.clone(),
                on_select: move |path| on_file_select.call(path),
            }

            // Diff content (right panel)
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
                    FileDiffContent { file: file_diff.clone() }
                } else {
                    NoFileSelectedState {}
                }
            }
        }
    }
}

/// Displays the diff content for a single file.
#[component]
fn FileDiffContent(file: FileDiff) -> Element {
    let (status_text, status_class) = match file.status {
        FileStatus::Added => ("Added", "text-green-400 bg-green-500/10"),
        FileStatus::Modified => ("Modified", "text-blue-400 bg-blue-500/10"),
        FileStatus::Deleted => ("Deleted", "text-red-400 bg-red-500/10"),
        FileStatus::Renamed => ("Renamed", "text-purple-400 bg-purple-500/10"),
    };

    // Derive language from file extension for syntax highlighting
    let language = language_from_path(&file.path).to_string();

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
                        DiffLineRow {
                            key: "{hunk_idx}-{line_idx}",
                            line: line.clone(),
                            show_line_numbers: true,
                            language: language.clone(),
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

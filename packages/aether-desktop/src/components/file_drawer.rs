//! File drawer component for displaying changed files in the diff view.

use dioxus::prelude::*;

use crate::state::{FileDiff, FileStatus};

/// Left panel showing the list of changed files.
#[component]
pub fn FileDrawer(
    files: Vec<FileDiff>,
    selected: Option<String>,
    on_select: EventHandler<String>,
) -> Element {
    let file_count = files.len();

    // Count file statuses in a single pass
    let (added_count, modified_count, deleted_count) =
        files
            .iter()
            .fold((0, 0, 0), |(added, modified, deleted), f| match f.status {
                FileStatus::Added => (added + 1, modified, deleted),
                FileStatus::Modified => (added, modified + 1, deleted),
                FileStatus::Deleted => (added, modified, deleted + 1),
                FileStatus::Renamed => (added, modified, deleted),
            });

    let files_label = if file_count == 1 { "file" } else { "files" };

    rsx! {
        div {
            class: "w-64 flex-shrink-0 border-r border-[#2d313a] bg-[#0f1116] flex flex-col h-full",

            // Header with file count summary
            div {
                class: "p-3 border-b border-[#2d313a]",
                div {
                    class: "text-sm font-medium text-white mb-2",
                    "Changed Files"
                }
                div {
                    class: "flex gap-3 text-xs",
                    if added_count > 0 {
                        span {
                            class: "text-green-400",
                            "+{added_count}"
                        }
                    }
                    if modified_count > 0 {
                        span {
                            class: "text-blue-400",
                            "~{modified_count}"
                        }
                    }
                    if deleted_count > 0 {
                        span {
                            class: "text-red-400",
                            "-{deleted_count}"
                        }
                    }
                    span {
                        class: "text-gray-500",
                        "{file_count} {files_label}"
                    }
                }
            }

            // File list
            div {
                class: "flex-1 overflow-y-auto",

                if files.is_empty() {
                    div {
                        class: "p-4 text-sm text-gray-500 text-center",
                        "No changes detected"
                    }
                }

                for file in files.iter() {
                    FileItem {
                        key: "{file.path}",
                        file: file.clone(),
                        is_selected: selected.as_ref() == Some(&file.path),
                        on_click: move |path: String| on_select.call(path),
                    }
                }
            }
        }
    }
}

/// Individual file item in the drawer.
#[component]
fn FileItem(file: FileDiff, is_selected: bool, on_click: EventHandler<String>) -> Element {
    let (status_icon, status_color) = match file.status {
        FileStatus::Added => ("+", "text-green-400"),
        FileStatus::Modified => ("~", "text-blue-400"),
        FileStatus::Deleted => ("-", "text-red-400"),
        FileStatus::Renamed => ("R", "text-purple-400"),
    };

    let bg_class = if is_selected {
        "bg-[#252830]"
    } else {
        "hover:bg-white/5"
    };

    // Extract filename and directory from path
    let (dir, filename) = match file.path.rsplit_once('/') {
        Some((d, f)) => (Some(d), f),
        None => (None, file.path.as_str()),
    };

    rsx! {
        button {
            class: "w-full px-3 py-2 flex items-center gap-2 text-left cursor-pointer transition-colors {bg_class}",
            onclick: {
                let path = file.path.clone();
                move |_| on_click.call(path.clone())
            },

            // Status indicator
            span {
                class: "w-4 text-center font-mono text-sm {status_color}",
                "{status_icon}"
            }

            // File info
            div {
                class: "flex-1 min-w-0",
                div {
                    class: "text-sm text-gray-200 truncate",
                    "{filename}"
                }
                if let Some(directory) = dir {
                    div {
                        class: "text-xs text-gray-500 truncate",
                        "{directory}"
                    }
                }
            }
        }
    }
}

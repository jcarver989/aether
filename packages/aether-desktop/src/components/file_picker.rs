//! File picker dropdown component.
//!
//! Displays a filterable list of files when the user types "@".

use crate::file_search::FileMatch;
use dioxus::prelude::*;

/// Props for the file picker dropdown.
#[component]
pub fn FilePicker(
    /// Filtered file matches to display
    matches: Vec<FileMatch>,
    /// Currently selected index in the list
    selected_index: usize,
    /// Whether the file list is still loading
    loading: bool,
    /// Called when a file is selected
    on_select: EventHandler<FileMatch>,
) -> Element {
    if loading {
        return rsx! {
            div {
                class: "absolute bottom-full left-0 right-0 mb-2 bg-[#1a1d23] border border-[#373b47] rounded-xl shadow-2xl p-4 text-gray-400 text-sm",
                "Loading files..."
            }
        };
    }

    if matches.is_empty() {
        return rsx! {
            div {
                class: "absolute bottom-full left-0 right-0 mb-2 bg-[#1a1d23] border border-[#373b47] rounded-xl shadow-2xl p-4 text-gray-400 text-sm",
                "No matching files"
            }
        };
    }

    // Clamp selected index to valid range
    let selected_index = selected_index.min(matches.len().saturating_sub(1));

    rsx! {
        div {
            class: "absolute bottom-full left-0 right-0 mb-2 bg-[#1a1d23] border border-[#373b47] rounded-xl shadow-2xl overflow-hidden max-h-80 overflow-y-auto z-50",

            // Header
            div {
                class: "px-4 py-3 border-b border-[#2d313a] bg-[#252830] text-xs text-gray-500 font-semibold uppercase tracking-wide",
                "Files"
            }

            // File list
            for (index, file) in matches.iter().enumerate() {
                FileItem {
                    key: "{file.path}",
                    file: file.clone(),
                    is_selected: index == selected_index,
                    on_click: {
                        let file = file.clone();
                        move |_| on_select.call(file.clone())
                    },
                }
            }
        }
    }
}

#[component]
fn FileItem(file: FileMatch, is_selected: bool, on_click: EventHandler<()>) -> Element {
    let class_str = if is_selected {
        "px-4 py-2 cursor-pointer transition-colors bg-blue-600 border-l-2 border-blue-400"
    } else {
        "px-4 py-2 cursor-pointer transition-colors hover:bg-[#252830] border-l-2 border-transparent"
    };

    // Format file size for display
    let size_display = format_file_size(file.size);

    // Get file icon based on extension
    let icon = get_file_icon(&file.path);

    rsx! {
        div {
            class: "{class_str}",
            onclick: move |_| on_click.call(()),

            div {
                class: "flex items-center gap-3",
                span {
                    class: "text-gray-500 text-sm",
                    "{icon}"
                }
                span {
                    class: "text-gray-200 font-mono text-sm truncate flex-1",
                    "{file.path}"
                }
                span {
                    class: "text-gray-500 text-xs",
                    "{size_display}"
                }
            }
        }
    }
}

/// A pill displaying a selected file with a remove button.
#[component]
pub fn FilePill(
    /// The file to display
    file: FileMatch,
    /// Called when the remove button is clicked
    on_remove: EventHandler<()>,
) -> Element {
    // Get just the filename for compact display
    let filename = file
        .path
        .rsplit('/')
        .next()
        .unwrap_or(&file.path);

    rsx! {
        div {
            class: "inline-flex items-center gap-1 bg-blue-600/20 text-blue-400 border border-blue-600/30 rounded-lg px-2 py-1 text-sm",
            title: "{file.path}",

            span {
                class: "font-mono truncate max-w-32",
                "@{filename}"
            }
            button {
                class: "ml-1 text-blue-300 hover:text-white transition-colors",
                onclick: move |e| {
                    e.stop_propagation();
                    on_remove.call(());
                },
                "×"
            }
        }
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn get_file_icon(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "🦀",
        "js" | "jsx" => "📜",
        "ts" | "tsx" => "📘",
        "py" => "🐍",
        "go" => "🐹",
        "md" => "📝",
        "json" => "📋",
        "toml" | "yaml" | "yml" => "⚙️",
        "html" => "🌐",
        "css" | "scss" | "sass" => "🎨",
        "sql" => "🗄️",
        "sh" | "bash" | "zsh" => "💻",
        _ => "📄",
    }
}

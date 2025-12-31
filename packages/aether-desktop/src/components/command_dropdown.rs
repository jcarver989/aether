//! Slash command dropdown component.
//!
//! Displays a filterable list of available commands when the user types "/".

use crate::state::SlashCommand;
use dioxus::prelude::*;

/// Props for the command dropdown.
#[component]
pub fn CommandDropdown(
    /// All available commands for this agent
    commands: Vec<SlashCommand>,
    /// Current filter text (text after "/")
    filter: String,
    /// Currently selected index in filtered list
    selected_index: usize,
    /// Called when a command is selected
    on_select: EventHandler<SlashCommand>,
) -> Element {
    // Filter commands based on current filter text
    let filtered_commands: Vec<&SlashCommand> = commands
        .iter()
        .filter(|cmd| filter.is_empty() || cmd.name.to_lowercase().contains(&filter.to_lowercase()))
        .collect();

    if filtered_commands.is_empty() {
        return rsx! {
            div {
                class: "absolute bottom-full left-0 right-0 mb-2 bg-[#1a1d23] border border-[#373b47] rounded-xl shadow-2xl p-4 text-gray-400 text-sm",
                "No matching commands"
            }
        };
    }

    // Clamp selected index to valid range
    let selected_index = selected_index.min(filtered_commands.len().saturating_sub(1));

    rsx! {
        div {
            class: "absolute bottom-full left-0 right-0 mb-2 bg-[#1a1d23] border border-[#373b47] rounded-xl shadow-2xl overflow-hidden max-h-80 overflow-y-auto z-50",

            // Header
            div {
                class: "px-4 py-3 border-b border-[#2d313a] bg-[#252830] text-xs text-gray-500 font-semibold uppercase tracking-wide",
                "Slash Commands"
            }

            // Command list
            for (index, cmd) in filtered_commands.iter().enumerate() {
                CommandItem {
                    key: "{index}-{cmd.name}",
                    command: (*cmd).clone(),
                    is_selected: index == selected_index,
                    on_click: {
                        let cmd = (*cmd).clone();
                        move |_| on_select.call(cmd.clone())
                    },
                }
            }
        }
    }
}

#[component]
fn CommandItem(command: SlashCommand, is_selected: bool, on_click: EventHandler<()>) -> Element {
    let class_str = if is_selected {
        "px-4 py-3 cursor-pointer transition-colors bg-green-600 border-l-2 border-green-400"
    } else {
        "px-4 py-3 cursor-pointer transition-colors hover:bg-[#252830] border-l-2 border-transparent"
    };

    let command_display = format!("/{}", command.name);
    let hint_display = command.input_hint.as_ref().map(|h| format!("<{}>", h));

    rsx! {
        div {
            class: "{class_str}",
            onclick: move |_| on_click.call(()),

            div {
                class: "flex items-center gap-3",
                span {
                    class: "text-green-400 font-mono text-sm font-medium",
                    "{command_display}"
                }
                if let Some(hint) = &hint_display {
                    span {
                        class: "text-gray-500 text-xs font-mono",
                        "{hint}"
                    }
                }
            }

            div {
                class: "text-gray-400 text-xs mt-1 leading-relaxed",
                "{command.description}"
            }
        }
    }
}

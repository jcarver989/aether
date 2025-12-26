//! New agent form component.
//!
//! A simple form for creating a new agent session with a command line.

use dioxus::prelude::*;

use crate::settings::Settings;
use crate::state::AgentConfig;

const CUSTOM_SERVER: &str = "__custom__";

/// Inline form for creating a new agent, displayed in the main content area
#[component]
pub fn NewAgentForm(
    on_create: EventHandler<(AgentConfig, String)>,
    on_cancel: EventHandler<()>,
) -> Element {
    let settings: Signal<Settings> = use_context();

    // Get the first server name as default, or use custom
    let default_server = {
        let s = settings.read();
        s.agent_servers.keys().next().cloned()
    };

    let mut selected_server = use_signal(|| {
        default_server
            .clone()
            .unwrap_or_else(|| CUSTOM_SERVER.to_string())
    });
    let custom_command_line = use_signal(|| AgentConfig::default().command_line);
    let mut initial_message = use_signal(String::new);

    // Helper to get the actual command line from selection
    let get_command_line = move || {
        let server_name = selected_server.read();
        if *server_name == CUSTOM_SERVER {
            custom_command_line.read().clone()
        } else {
            settings
                .read()
                .agent_servers
                .get(&*server_name)
                .map(|c| c.to_command_line())
                .unwrap_or_else(|| custom_command_line.read().clone())
        }
    };

    let do_submit = move || {
        let config = AgentConfig {
            command_line: get_command_line(),
        };
        on_create.call((config, initial_message.read().clone()));
    };

    // Get sorted server names for dropdown
    let mut server_names: Vec<String> = settings.read().agent_servers.keys().cloned().collect();
    server_names.sort();

    rsx! {
        div {
            class: "flex-1 flex flex-col h-full bg-gray-950",

            // Top bar with close button
            div {
                class: "flex justify-end p-3",
                button {
                    class: "text-gray-500 hover:text-white transition-colors p-1",
                    onclick: move |_| on_cancel.call(()),
                    "X"
                }
            }

            // Centered prompt area
            div {
                class: "flex-1 flex items-center justify-center px-6 pb-24",

                div {
                    class: "w-full max-w-2xl",

                    // Main prompt box
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-xl",

                        // Textarea
                        textarea {
                            class: "w-full bg-transparent text-white px-4 pt-4 pb-2 resize-none focus:outline-none text-base placeholder-gray-500",
                            rows: "4",
                            placeholder: "What would you like to work on?",
                            value: "{initial_message}",
                            oninput: move |e| initial_message.set(e.value()),
                            onkeydown: move |e: KeyboardEvent| {
                                if e.key() == Key::Enter && !e.modifiers().shift() && !initial_message.read().trim().is_empty() {
                                    e.prevent_default();
                                    do_submit();
                                }
                            },
                        }

                        // Bottom toolbar
                        div {
                            class: "flex items-center justify-end px-3 py-2 border-t border-gray-800 gap-2",

                            // Agent server dropdown
                            select {
                                class: "bg-gray-800 text-white border border-gray-700 rounded-lg px-3 py-1.5 focus:outline-none focus:border-blue-500 text-sm",
                                value: "{selected_server}",
                                onchange: move |e| selected_server.set(e.value()),

                                for name in server_names.iter() {
                                    option {
                                        key: "{name}",
                                        value: "{name}",
                                        "{name}"
                                    }
                                }
                                option {
                                    value: CUSTOM_SERVER,
                                    "Custom..."
                                }
                            }

                            // Submit button
                            button {
                                class: "bg-blue-600 hover:bg-blue-700 disabled:bg-gray-700 disabled:text-gray-500 text-white px-4 py-1.5 rounded-lg transition-colors text-sm font-medium",
                                disabled: initial_message.read().trim().is_empty(),
                                onclick: move |_| do_submit(),
                                "Start"
                            }
                        }
                    }
                }
            }
        }
    }
}

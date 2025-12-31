//! Settings editor component.
//!
//! A structured form for editing agent server configurations.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::settings::{AgentServerConfig, Settings};

/// Settings editor displayed in the right pane
#[component]
pub fn SettingsEditor(on_close: EventHandler<()>) -> Element {
    let mut settings: Signal<Settings> = use_context();

    // Local copy for editing
    let mut draft = use_signal(|| settings.read().clone());
    let mut editing_server = use_signal(|| None::<String>);
    let mut error_message = use_signal(|| None::<String>);

    // For adding a new server
    let mut new_server_name = use_signal(String::new);

    let save_settings = move |_| {
        let new_settings = draft.read().clone();
        if let Some(path) = Settings::default_path() {
            match new_settings.save(&path) {
                Ok(()) => {
                    // Update the global settings
                    *settings.write() = new_settings;
                    error_message.set(None);
                    on_close.call(());
                }
                Err(e) => {
                    error_message.set(Some(format!("Failed to save: {}", e)));
                }
            }
        } else {
            error_message.set(Some("Could not determine settings path".to_string()));
        }
    };

    let add_server = move |_| {
        let name = new_server_name.read().trim().to_string();
        if name.is_empty() {
            return;
        }
        if draft.read().agent_servers.contains_key(&name) {
            error_message.set(Some(format!("Server '{}' already exists", name)));
            return;
        }
        draft
            .write()
            .agent_servers
            .insert(name.clone(), AgentServerConfig::new(""));
        editing_server.set(Some(name));
        new_server_name.set(String::new());
        error_message.set(None);
    };

    // Sort server names for consistent display
    let mut server_names: Vec<String> = draft.read().agent_servers.keys().cloned().collect();
    server_names.sort();

    rsx! {
        div {
            class: "flex-1 flex flex-col h-full bg-gray-950",

            // Header
            div {
                class: "flex items-center justify-between p-4 border-b border-gray-800",
                h2 { class: "text-xl font-semibold text-white", "Settings" }
                button {
                    class: "text-gray-500 hover:text-white transition-colors p-1",
                    onclick: move |_| on_close.call(()),
                    "X"
                }
            }

            // Content
            div {
                class: "flex-1 overflow-y-auto p-6",

                // Error message
                if let Some(err) = error_message.read().as_ref() {
                    div {
                        class: "mb-4 p-3 bg-red-900/50 border border-red-700 rounded-lg text-red-200 text-sm",
                        "{err}"
                    }
                }

                // Agent Servers section
                div {
                    class: "mb-6",
                    h3 { class: "text-lg font-medium text-gray-200 mb-4", "Agent Servers" }
                    p { class: "text-sm text-gray-500 mb-4", "Configure the agent servers available when creating new agents." }

                    // Server list
                    div {
                        class: "space-y-3",
                        for name in server_names {
                            ServerCard {
                                key: "{name}",
                                name: name.clone(),
                                draft: draft,
                                editing_server: editing_server,
                            }
                        }
                    }

                    // Add new server
                    div {
                        class: "mt-4 flex gap-2",
                        input {
                            r#type: "text",
                            class: "flex-1 bg-gray-800 text-white border border-gray-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-green-500 placeholder-gray-500",
                            placeholder: "New server name (e.g., claude, ollama)",
                            value: "{new_server_name}",
                            oninput: move |e| new_server_name.set(e.value()),
                            onkeydown: move |e: KeyboardEvent| {
                                if e.key() == Key::Enter {
                                    let name = new_server_name.read().trim().to_string();
                                    if name.is_empty() {
                                        return;
                                    }
                                    if draft.read().agent_servers.contains_key(&name) {
                                        error_message.set(Some(format!("Server '{}' already exists", name)));
                                        return;
                                    }
                                    draft
                                        .write()
                                        .agent_servers
                                        .insert(name.clone(), AgentServerConfig::new(""));
                                    editing_server.set(Some(name));
                                    new_server_name.set(String::new());
                                    error_message.set(None);
                                }
                            },
                        }
                        button {
                            class: "bg-gray-700 hover:bg-gray-600 text-white px-4 py-2 rounded-lg text-sm transition-colors",
                            onclick: add_server,
                            "+ Add"
                        }
                    }
                }
            }

            // Footer with save/cancel
            div {
                class: "flex justify-end gap-3 p-4 border-t border-gray-800",
                button {
                    class: "px-4 py-2 text-gray-400 hover:text-white transition-colors",
                    onclick: move |_| on_close.call(()),
                    "Cancel"
                }
                button {
                    class: "bg-green-600 hover:bg-green-700 text-black px-4 py-2 rounded-lg transition-colors",
                    onclick: save_settings,
                    "Save"
                }
            }
        }
    }
}

/// A single server card with edit/delete functionality
#[component]
fn ServerCard(
    name: String,
    draft: Signal<Settings>,
    mut editing_server: Signal<Option<String>>,
) -> Element {
    let is_editing = editing_server.read().as_ref() == Some(&name);
    let config = draft
        .read()
        .agent_servers
        .get(&name)
        .cloned()
        .unwrap_or_else(|| AgentServerConfig::new(""));

    let name_for_toggle = name.clone();
    let name_for_delete = name.clone();

    rsx! {
        div {
            class: "bg-gray-900 border border-gray-700 rounded-lg",

            // Server header
            div {
                class: "flex items-center justify-between p-3",
                div {
                    class: "flex-1 min-w-0",
                    div { class: "font-medium text-gray-200", "{name}" }
                    div { class: "text-xs text-gray-500 font-mono truncate mt-1", "{config.to_command_line()}" }
                }
                div {
                    class: "flex items-center gap-2 ml-2",
                    button {
                        class: "text-sm text-green-400 hover:text-green-300 px-2 py-1",
                        onclick: move |_| {
                            if is_editing {
                                editing_server.set(None);
                            } else {
                                editing_server.set(Some(name_for_toggle.clone()));
                            }
                        },
                        if is_editing { "Done" } else { "Edit" }
                    }
                    button {
                        class: "text-sm text-red-400 hover:text-red-300 px-2 py-1",
                        onclick: move |_| {
                            draft.write().agent_servers.remove(&name_for_delete);
                            if editing_server.read().as_ref() == Some(&name_for_delete) {
                                editing_server.set(None);
                            }
                        },
                        "Delete"
                    }
                }
            }

            // Edit form (expanded when editing)
            if is_editing {
                ServerEditForm {
                    name: name.clone(),
                    draft: draft,
                }
            }
        }
    }
}

/// Form for editing a single server's configuration
#[component]
fn ServerEditForm(name: String, mut draft: Signal<Settings>) -> Element {
    let config = draft
        .read()
        .agent_servers
        .get(&name)
        .cloned()
        .unwrap_or_else(|| AgentServerConfig::new(""));

    let mut command = use_signal(|| config.command.clone());
    let mut args_text = use_signal(|| config.args.join("\n"));
    let mut env_text = use_signal(|| {
        config
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    });

    // Helper to build config from current field values
    let build_config = move || AgentServerConfig {
        command: command.read().clone(),
        args: args_text
            .read()
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        env: env_text
            .read()
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                let mut parts = line.splitn(2, '=');
                let key = parts.next()?.trim().to_string();
                let value = parts.next().unwrap_or("").trim().to_string();
                if key.is_empty() {
                    return None;
                }
                Some((key, value))
            })
            .collect::<HashMap<_, _>>(),
    };

    let name_for_cmd = name.clone();
    let name_for_args = name.clone();
    let name_for_env = name.clone();

    rsx! {
        div {
            class: "p-3 pt-0 space-y-3 border-t border-gray-800",

            // Command
            div {
                label { class: "block text-sm text-gray-400 mb-1", "Command" }
                input {
                    r#type: "text",
                    class: "w-full bg-gray-800 text-white border border-gray-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-green-500 placeholder-gray-500 font-mono",
                    placeholder: "e.g., aether-acp, claude, /usr/local/bin/ollama",
                    value: "{command}",
                    oninput: move |e| {
                        command.set(e.value());
                        let new_config = build_config();
                        draft.write().agent_servers.insert(name_for_cmd.clone(), new_config);
                    },
                }
            }

            // Arguments
            div {
                label { class: "block text-sm text-gray-400 mb-1", "Arguments (one per line)" }
                textarea {
                    class: "w-full bg-gray-800 text-white border border-gray-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-green-500 placeholder-gray-500 font-mono resize-none",
                    rows: "3",
                    placeholder: "--model\nanthropic:claude-sonnet-4\n--mcp-config\nmcp.json",
                    value: "{args_text}",
                    oninput: move |e| {
                        args_text.set(e.value());
                        let new_config = build_config();
                        draft.write().agent_servers.insert(name_for_args.clone(), new_config);
                    },
                }
            }

            // Environment variables
            div {
                label { class: "block text-sm text-gray-400 mb-1", "Environment Variables (KEY=value, one per line)" }
                textarea {
                    class: "w-full bg-gray-800 text-white border border-gray-700 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-green-500 placeholder-gray-500 font-mono resize-none",
                    rows: "2",
                    placeholder: "ANTHROPIC_API_KEY=sk-...\nOPENAI_API_KEY=sk-...",
                    value: "{env_text}",
                    oninput: move |e| {
                        env_text.set(e.value());
                        let new_config = build_config();
                        draft.write().agent_servers.insert(name_for_env.clone(), new_config);
                    },
                }
            }
        }
    }
}

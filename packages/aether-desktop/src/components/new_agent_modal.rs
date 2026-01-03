//! New agent form component.
//!
//! A simple form for creating a new agent session with a command line.

use crate::components::layout::{Inline, Space};
use crate::settings::Settings;
use crate::state::{AgentConfig, ExecutionMode};
use dioxus::prelude::*;
use std::path::PathBuf;

const CUSTOM_SERVER: &str = "__custom__";

/// Check if a Dockerfile exists at .aether/Dockerfile in the given directory.
fn dockerfile_path_if_exists(cwd: &std::path::Path) -> Option<PathBuf> {
    let path = cwd.join(".aether/Dockerfile");
    if path.exists() { Some(path) } else { None }
}

/// Inline form for creating a new agent, displayed in the main content area
#[component]
pub fn NewAgentForm(
    on_create: EventHandler<(AgentConfig, String)>,
    on_cancel: EventHandler<()>,
) -> Element {
    let settings: Signal<Settings> = use_context();

    let mut selected_server = use_signal(|| {
        let s = settings.read();
        s.agent_servers.keys().next().cloned()
            .unwrap_or_else(|| CUSTOM_SERVER.to_string())
    });
    let custom_command_line = use_signal(|| AgentConfig::default().command_line);
    let mut initial_message = use_signal(String::new);

    // Check for .aether/Dockerfile in current directory
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let dockerfile_path = dockerfile_path_if_exists(&cwd);
    let has_dockerfile = dockerfile_path.is_some();

    // Docker mode toggle (enabled by default when Dockerfile exists)
    let mut use_docker = use_signal(|| has_dockerfile);

    // Helper to get command line from selection
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

    // Helper to get display name from selection
    let get_display_name = move || {
        let server_name = selected_server.read();
        if *server_name == CUSTOM_SERVER {
            command_basename(&custom_command_line.read())
        } else {
            capitalize(&server_name)
        }
    };

    // Get sorted server names for dropdown
    let mut server_names: Vec<String> = settings.read().agent_servers.keys().cloned().collect();
    server_names.sort();

    rsx! {
        div {
            class: "flex-1 flex flex-col h-full bg-bg-primary",

            // Top bar with close button
            div {
                class: "flex justify-end p-3",
                button {
                    class: "text-gray-500 hover:text-white transition-all p-1 rounded-lg hover:bg-white/10 hover:shadow-md",
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
                        class: "bg-bg-secondary border border-border-default rounded-2xl shadow-xl overflow-hidden",

                        // Textarea
                        textarea {
                            class: "w-full bg-transparent text-white px-5 pt-5 pb-3 resize-none focus:outline-none text-base placeholder-gray-500 leading-relaxed",
                            rows: "4",
                            placeholder: "What would you like to work on?",
                            value: "{initial_message}",
                            oninput: move |e| initial_message.set(e.value()),
                            onkeydown: {
                                let dockerfile_path_for_keydown = dockerfile_path.clone();
                                move |e: KeyboardEvent| {
                                    if e.key() == Key::Enter && !e.modifiers().shift() && !initial_message.read().trim().is_empty() {
                                        e.prevent_default();
                                        submit_form(
                                            &on_create,
                                            get_display_name(),
                                            get_command_line(),
                                            use_docker(),
                                            dockerfile_path_for_keydown.as_ref(),
                                            &initial_message.read(),
                                        );
                                    }
                                }
                            },
                        }

                        // Bottom toolbar
                        Inline {
                            gap: Space::S3,
                            class: "justify-between px-4 py-3 border-t border-border-subtle",

                            // Left side: Docker checkbox (only shown if Dockerfile exists)
                            div {
                                class: "flex items-center gap-2",
                                if has_dockerfile {
                                    label {
                                        class: "flex items-center gap-2 text-sm text-gray-300 cursor-pointer",
                                        input {
                                            r#type: "checkbox",
                                            class: "w-4 h-4 rounded border-border-default bg-bg-tertiary accent-green-500 cursor-pointer",
                                            checked: use_docker(),
                                            onchange: move |e| use_docker.set(e.checked()),
                                        }
                                        "Use Dockerfile"
                                    }
                                }
                            }

                            // Right side: server dropdown and submit button
                            div {
                                class: "flex items-center gap-3",

                                // Agent server dropdown
                                select {
                                    class: "bg-bg-tertiary text-white border border-border-default rounded-lg px-3 py-2 focus:outline-none focus:border-green-500 text-sm hover:border-[#64748b] transition-colors cursor-pointer",
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
                                    class: "btn-primary px-5 py-2 rounded-lg text-sm font-semibold disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100",
                                    disabled: initial_message.read().trim().is_empty(),
                                    onclick: {
                                        let dockerfile_path_for_click = dockerfile_path.clone();
                                        move |_| {
                                            submit_form(
                                                &on_create,
                                                get_display_name(),
                                                get_command_line(),
                                                use_docker(),
                                                dockerfile_path_for_click.as_ref(),
                                                &initial_message.read(),
                                            );
                                        }
                                    },
                                    "Start"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Extract basename from a command line (first word, last path component).
fn command_basename(cmd: &str) -> String {
    let first_word = cmd.split_whitespace().next().unwrap_or(cmd);
    let basename = first_word.rsplit('/').next().unwrap_or(first_word);
    capitalize(basename)
}

/// Capitalize the first letter of a string.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

/// Helper to submit the form with the current configuration.
fn submit_form(
    on_create: &EventHandler<(AgentConfig, String)>,
    name: String,
    command_line: String,
    use_docker: bool,
    dockerfile_path: Option<&PathBuf>,
    initial_message: &str,
) {
    let execution_mode = if use_docker {
        if let Some(path) = dockerfile_path {
            ExecutionMode::Docker {
                dockerfile_path: path.clone(),
            }
        } else {
            ExecutionMode::Local
        }
    } else {
        ExecutionMode::Local
    };

    let config = AgentConfig {
        name,
        command_line,
        execution_mode,
    };
    on_create.call((config, initial_message.to_string()));
}

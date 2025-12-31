//! Agent view component.
//!
//! Displays the chat interface for a single agent session.

use super::command_dropdown::CommandDropdown;
use super::diff_view::DiffView;
use super::file_picker::{FilePicker, FilePill};
use super::message_bubble::MessageBubble;
use super::prompt_input::PromptInput;
use super::view_tabs::{AgentViewTab, ViewTabs};
use crate::hooks::{AgentChatController, use_agent_chat};
use crate::state::{AgentStatus, CommentKey, DiffComment, Message, MessageKind, Role, now_iso};
use crate::state::{AgentStatus, CommentKey, DiffComment, Message, MessageKind, Role, now_iso};
use crate::{HANDLES, with_agent_mut};
use crate::{HANDLES, with_agent_mut};
use agent_client_protocol::ContentBlock;
use dioxus::prelude::*;

#[component]
pub fn AgentView(agent_id: String) -> Element {
    let mut active_tab = use_signal(|| AgentViewTab::Chat);

    let Some(chat) = use_agent_chat(&agent_id) else {
        return rsx! {
            div {
                class: "flex-1 flex items-center justify-center text-gray-500",
                "Agent not found"
            }
        };
    };

    let Some(agent_signal) = chat.agent() else {
        return rsx! {
            div {
                class: "flex-1 flex items-center justify-center text-gray-500",
                "Agent not found"
            }
        };
    };

    let agent = agent_signal.read();
    let is_running = chat.is_running();
    let status_text = match &agent.status {
        AgentStatus::Idle => "Idle".to_string(),
        AgentStatus::Starting(phase) => phase.text().to_string(),
        AgentStatus::Running => "Running...".to_string(),
        AgentStatus::Error(_) => "Error".to_string(),
    };
    let status_color = match &agent.status {
        AgentStatus::Idle => "bg-gray-600 text-gray-300",
        AgentStatus::Starting(_) => "bg-yellow-600/20 text-yellow-400 border border-yellow-600/30",
        AgentStatus::Running => "bg-green-600/20 text-green-400 border border-green-600/30",
        AgentStatus::Error(_) => "bg-red-600/20 text-red-400 border border-red-600/30",
    };

    let diff_state = agent.diff_state.clone();
    let messages = agent.messages.clone();
    let agent_name = agent.name.clone();
    let command_line = agent.config.command_line.clone();

    // Clone agent_id for closures
    let agent_id_for_diff = agent_id.clone();

    rsx! {
        div {
            class: "flex-1 flex flex-col h-full bg-bg-primary overflow-hidden",

            // Header
            div {
                class: "p-4 border-b border-border-subtle flex items-center justify-between",
                Inline {
                    gap: Space::S4,
                    div {
                        h2 { class: "text-lg font-semibold text-white tracking-tight", "{agent_name}" }
                        p { class: "text-sm text-gray-500 font-mono truncate max-w-xs", "{command_line}" }
                    }
                    ViewTabs {
                        active: active_tab(),
                        on_change: move |tab| active_tab.set(tab),
                    }
                }
                span {
                    class: "px-3 py-1 rounded-full text-xs font-medium {status_color}",
                    "{status_text}"
                }
            }

            // Content area
            match active_tab() {
                AgentViewTab::Chat => rsx! {
                    // Message list
                    Stack {
                        gap: Space::S1,
                        p: Space::S3,
                        class: "flex-1 overflow-y-auto",
                        id: "message-list",

                        if messages.is_empty() {
                            div {
                                class: "h-full flex items-center justify-center text-gray-500",
                                "Send a message to start the conversation"
                            }
                        }

                        for msg in messages.iter() {
                            MessageBubble {
                                key: "{msg.id}",
                                message: msg.clone(),
                            }
                        }

                        div { id: "message-end" }
                    }

                    // Input area
                    ChatInput { chat, is_running }
                },
                AgentViewTab::Diff => {
                    let agent_id = agent_id_for_diff.clone();

                    rsx! {
                        div {
                            class: "flex-1 overflow-hidden",
                            DiffView {
                                diff_state: diff_state,
                                on_file_select: {
                                    let agent_id = agent_id.clone();
                                    move |path: String| {
                                        with_agent_mut(&agent_id, |agent| {
                                            agent.diff_state.selected_file = Some(path);
                                        });
                                    }
                                },
                                on_add_comment: {
                                    let agent_id = agent_id.clone();
                                    move |comment: DiffComment| {
                                        with_agent_mut(&agent_id, |agent| {
                                            agent.diff_state.add_comment(comment);
                                        });
                                    }
                                },
                                on_edit_comment: {
                                    let agent_id = agent_id.clone();
                                    move |(key, new_content): (CommentKey, String)| {
                                        with_agent_mut(&agent_id, |agent| {
                                            agent.diff_state.update_comment(&key, new_content);
                                        });
                                    }
                                },
                                on_remove_comment: {
                                    let agent_id = agent_id.clone();
                                    move |key: CommentKey| {
                                        with_agent_mut(&agent_id, |agent| {
                                            agent.diff_state.remove_comment(&key);
                                        });
                                    }
                                },
                                on_clear_comments: {
                                    let agent_id = agent_id.clone();
                                    move |_| {
                                        with_agent_mut(&agent_id, |agent| {
                                            agent.diff_state.clear_comments();
                                        });
                                    }
                                },
                                on_send_comments: {
                                    let agent_id = agent_id.clone();
                                    let mut active_tab = active_tab;
                                    move |prompt: String| {
                                        with_agent_mut(&agent_id, |agent| {
                                            agent.messages.push(Message {
                                                id: uuid::Uuid::new_v4().to_string(),
                                                role: Role::User,
                                                content: prompt.clone(),
                                                kind: MessageKind::Text,
                                                timestamp: now_iso(),
                                                is_streaming: false,
                                            });
                                            agent.status = AgentStatus::Running;
                                            agent.diff_state.clear_comments();
                                        });

                                        if let Err(e) = HANDLES.read().send_prompt(&agent_id, vec![ContentBlock::from(prompt)]) {
                                            tracing::error!("Failed to send comment prompt: {}", e);
                                            with_agent_mut(&agent_id, |agent| {
                                                agent.status = AgentStatus::Error(e.to_string());
                                            });
                                        }

                                        active_tab.set(AgentViewTab::Chat);
                                    }
                                },
                            }
                        }
                    }
                },
            }
        }
    }
}

/// Chat input component with autocomplete dropdowns.
#[component]
fn ChatInput(mut chat: AgentChatController, is_running: bool) -> Element {
    use crate::hooks::InputMode;

    // Read state from controller
    let pending_files = chat.pending_files.read().clone();
    let input_value = chat.input_value();
    let input_mode = chat.input_mode();
    let files_loading = *chat.files_loading.read();
    let available_commands = chat.available_commands();

    rsx! {
        div {
            class: "p-4 border-t border-border-subtle bg-bg-secondary",

            // File pills (pending file mentions)
            if !pending_files.is_empty() {
                Inline {
                    gap: Space::S2,
                    class: "flex-wrap mb-3",
                    for file in pending_files.iter() {
                        FilePill {
                            key: "{file.path}",
                            file: file.clone(),
                            on_remove: {
                                let path = file.path.clone();
                                let mut chat = chat;
                                move |_| {
                                    chat.remove_pending_file(&path);
                                }
                            },
                        }
                    }
                }
            }

            // Relative container for dropdown positioning
            div {
                class: "relative",

                // Autocomplete dropdown based on input mode
                match &input_mode {
                    InputMode::SlashCommand(ctrl) => rsx! {
                        CommandDropdown {
                            commands: available_commands.clone(),
                            filter: ctrl.filter(),
                            selected_index: ctrl.selected_index(),
                            on_select: {
                                let mut chat = chat;
                                move |cmd: crate::state::SlashCommand| {
                                    chat.input.set(format!("/{} ", cmd.name));
                                    chat.input_mode.set(InputMode::Normal);
                                }
                            },
                        }
                    },
                    InputMode::FileMention(ctrl) => rsx! {
                        FilePicker {
                            matches: ctrl.items(),
                            selected_index: ctrl.selected_index(),
                            loading: files_loading,
                            on_select: {
                                let mut chat = chat;
                                move |file: crate::file_search::FileMatch| {
                                    // Add file to pending
                                    {
                                        let mut files = chat.pending_files.write();
                                        if !files.iter().any(|f| f.path == file.path) {
                                            files.push(file);
                                        }
                                    }
                                    // Remove @query from input
                                    let current = chat.input.read().clone();
                                    if let Some(at_pos) = current.rfind('@') {
                                        chat.input.set(current[..at_pos].to_string());
                                    }
                                    chat.input_mode.set(InputMode::Normal);
                                }
                            },
                        }
                    },
                    InputMode::Normal => rsx! {},
                }

                PromptInput {
                    value: chat.input,
                    on_change: move |value: String| {
                        chat.on_input_change(value);
                    },
                    on_submit: move |_| {
                        chat.send();
                    },
                    placeholder: "Type a message, / for commands, or @ to mention files...".to_string(),
                    disabled: is_running,
                    rows: "2",
                }
            }
        }
    }
}

#[component]
pub fn EmptyState() -> Element {
    rsx! {
        Stack {
            gap: Space::S4,
            class: "flex-1 items-center justify-center text-gray-500 bg-bg-primary",
            div {
                class: "w-20 h-20 mb-2 rounded-full bg-gradient-to-br from-blue-500/20 to-purple-500/20 flex items-center justify-center",
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "40",
                    height: "40",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "text-gray-400",
                    path {
                        d: "M12 5v14M5 12h14"
                    }
                }
            }
            Stack {
                gap: Space::S2,
                class: "items-center",
                p { class: "text-lg font-medium text-gray-400", "Create a new agent to get started" }
                p { class: "text-sm text-gray-600", "Click the \"New Agent\" button in the sidebar" }
            }
        }
    }
}

//! Agent view component.
//!
//! Displays the chat interface for a single agent session.

use dioxus::prelude::*;

use crate::state::{now_iso, AgentStatus, Message, MessageKind, Role, SlashCommand};
use crate::{AGENTS, HANDLES};

use super::command_dropdown::CommandDropdown;
use super::message_bubble::MessageBubble;

/// State for the command dropdown
#[derive(Clone, PartialEq, Default)]
struct DropdownState {
    visible: bool,
    selected_index: usize,
    filter_text: String,
}

#[component]
pub fn AgentView(agent_id: String) -> Element {
    let mut input_val = use_signal(String::new);
    let dropdown_state = use_signal(DropdownState::default);
    let agent_id_for_send = agent_id.clone();
    let agent_id_for_handlers = agent_id.clone();

    // Get available commands for this agent (read once per render)
    let available_commands: Vec<SlashCommand> = {
        let agents = AGENTS.read();
        agents
            .iter()
            .find(|a| a.id == agent_id_for_handlers)
            .map(|a| a.available_commands.clone())
            .unwrap_or_default()
    };

    let mut do_send = {
        let mut dropdown_state = dropdown_state;
        move || {
            let content = input_val.read().clone();
            if content.trim().is_empty() {
                return;
            }

            // Close dropdown on send
            dropdown_state.write().visible = false;

            // Add user message to state
            {
                let mut list = AGENTS.write();
                if let Some(agent) = list.iter_mut().find(|a| a.id == agent_id_for_send) {
                    agent.messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: Role::User,
                        content: content.clone(),
                        kind: MessageKind::Text,
                        timestamp: now_iso(),
                        is_streaming: false,
                    });
                    agent.status = AgentStatus::Running;
                }
            }

            // Send via handles (separate from UI state)
            if let Err(e) = HANDLES.read().send_prompt(&agent_id_for_send, content) {
                tracing::error!("Failed to send message: {}", e);
                let mut list = AGENTS.write();
                if let Some(agent) = list.iter_mut().find(|a| a.id == agent_id_for_send) {
                    agent.status = AgentStatus::Error(e.to_string());
                }
            }

            input_val.set(String::new());
        }
    };

    // Handle command selection from dropdown
    let on_command_select = {
        let mut input_val = input_val;
        let mut dropdown_state = dropdown_state;
        move |cmd: SlashCommand| {
            // Replace input with "/{command} "
            input_val.set(format!("/{} ", cmd.name));
            dropdown_state.write().visible = false;
        }
    };

    // Handle input changes - detect "/" for dropdown
    let on_input_change = {
        let mut dropdown_state = dropdown_state;
        let commands = available_commands.clone();
        move |e: Event<FormData>| {
            let value = e.value();
            input_val.set(value.clone());

            // Check if we should show/hide dropdown
            if value.starts_with('/') && !value.contains(' ') {
                // Show dropdown, filter by text after "/"
                let filter = value.trim_start_matches('/').to_string();
                let mut state = dropdown_state.write();
                state.visible = !commands.is_empty();
                state.filter_text = filter;
                state.selected_index = 0;
            } else {
                dropdown_state.write().visible = false;
            }
        }
    };

    // Enhanced keyboard handling
    let on_keydown = {
        let mut do_send = do_send.clone();
        let mut dropdown_state = dropdown_state;
        let commands = available_commands.clone();
        let mut input_val = input_val;

        move |e: KeyboardEvent| {
            let state = dropdown_state.read().clone();

            if state.visible {
                // Dropdown is open - handle navigation
                let filtered: Vec<&SlashCommand> = commands
                    .iter()
                    .filter(|cmd| {
                        state.filter_text.is_empty()
                            || cmd
                                .name
                                .to_lowercase()
                                .contains(&state.filter_text.to_lowercase())
                    })
                    .collect();

                match e.key() {
                    Key::ArrowDown => {
                        e.prevent_default();
                        let mut state = dropdown_state.write();
                        if !filtered.is_empty() {
                            state.selected_index = (state.selected_index + 1) % filtered.len();
                        }
                    }
                    Key::ArrowUp => {
                        e.prevent_default();
                        let mut state = dropdown_state.write();
                        if !filtered.is_empty() {
                            state.selected_index = state
                                .selected_index
                                .checked_sub(1)
                                .unwrap_or(filtered.len().saturating_sub(1));
                        }
                    }
                    Key::Enter => {
                        e.prevent_default();
                        if let Some(cmd) = filtered.get(state.selected_index) {
                            input_val.set(format!("/{} ", cmd.name));
                            dropdown_state.write().visible = false;
                        }
                    }
                    Key::Escape => {
                        e.prevent_default();
                        dropdown_state.write().visible = false;
                    }
                    Key::Tab => {
                        // Tab also selects (like autocomplete)
                        e.prevent_default();
                        if let Some(cmd) = filtered.get(state.selected_index) {
                            input_val.set(format!("/{} ", cmd.name));
                            dropdown_state.write().visible = false;
                        }
                    }
                    _ => {}
                }
            } else {
                // Normal mode - send on Enter
                if e.key() == Key::Enter && !e.modifiers().shift() {
                    e.prevent_default();
                    do_send();
                }
            }
        }
    };

    // Read from global signal during render - this subscribes the component
    // to AGENTS changes for reactive re-renders when messages stream in
    let agents = AGENTS.read();
    tracing::debug!("AgentView rendering, agents count: {}", agents.len());
    let Some(agent) = agents.iter().find(|a| a.id == agent_id) else {
        return rsx! {
            div {
                class: "flex-1 flex items-center justify-center text-gray-500",
                "Agent not found"
            }
        };
    };

    let is_running = matches!(agent.status, AgentStatus::Running);
    let status_text = match &agent.status {
        AgentStatus::Idle => "Idle",
        AgentStatus::Running => "Running...",
        AgentStatus::Error(_) => "Error",
    };
    let status_color = match &agent.status {
        AgentStatus::Idle => "bg-gray-600 text-gray-300",
        AgentStatus::Running => "bg-green-600/20 text-green-400 border border-green-600/30",
        AgentStatus::Error(_) => "bg-red-600/20 text-red-400 border border-red-600/30",
    };

    let dropdown_visible = dropdown_state.read().visible;
    let dropdown_selected = dropdown_state.read().selected_index;
    let dropdown_filter = dropdown_state.read().filter_text.clone();

    rsx! {
        div {
            class: "flex-1 flex flex-col h-full bg-[#0f1116]",

            // Header with agent name and status
            div {
                class: "p-4 border-b border-[#2d313a] flex items-center justify-between",
                div {
                    h2 { class: "text-lg font-semibold text-white tracking-tight", "{agent.name}" }
                    p { class: "text-sm text-gray-500 font-mono truncate max-w-xs", "{agent.config.command_line}" }
                }
                span {
                    class: "px-3 py-1.5 rounded-full text-xs font-medium {status_color}",
                    "{status_text}"
                }
            }

            // Message list
            div {
                class: "flex-1 overflow-y-auto p-4 space-y-4",
                id: "message-list",

                if agent.messages.is_empty() {
                    div {
                        class: "h-full flex items-center justify-center text-gray-500",
                        "Send a message to start the conversation"
                    }
                }

                for msg in agent.messages.iter() {
                    MessageBubble {
                        key: "{msg.id}",
                        message: msg.clone(),
                    }
                }

                // Scroll anchor
                div { id: "message-end" }
            }

            // Input area with dropdown
            div {
                class: "p-4 border-t border-[#2d313a] bg-[#1a1d23]",

                // Relative container for dropdown positioning
                div {
                    class: "relative",

                    // Command dropdown (positioned above input)
                    if dropdown_visible && !available_commands.is_empty() {
                        CommandDropdown {
                            commands: available_commands.clone(),
                            filter: dropdown_filter,
                            selected_index: dropdown_selected,
                            on_select: on_command_select,
                        }
                    }

                    div {
                        class: "flex gap-3",
                        textarea {
                            class: "input-field flex-1 rounded-xl px-4 py-3 resize-none",
                            value: "{input_val}",
                            oninput: on_input_change,
                            onkeydown: on_keydown,
                            placeholder: "Type a message or / for commands... (Enter to send)",
                            disabled: is_running,
                            rows: "2",
                        }
                        button {
                            class: "btn-primary px-6 py-3 rounded-xl font-semibold disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100",
                            onclick: move |_| do_send(),
                            disabled: is_running,
                            if is_running {
                                "Working..."
                            } else {
                                "Send"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn EmptyState() -> Element {
    rsx! {
        div {
            class: "flex-1 flex flex-col items-center justify-center text-gray-500 bg-[#0f1116]",
            div {
                class: "w-20 h-20 mb-6 rounded-full bg-gradient-to-br from-blue-500/20 to-purple-500/20 flex items-center justify-center",
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
            p { class: "text-lg font-medium text-gray-400", "Create a new agent to get started" }
            p { class: "text-sm mt-2 text-gray-600", "Click the \"New Agent\" button in the sidebar" }
        }
    }
}

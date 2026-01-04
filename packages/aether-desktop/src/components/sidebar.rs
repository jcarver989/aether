use dioxus::prelude::*;

use crate::components::ContextProgressBar;
use crate::components::layout::{Inline, Space, Stack};
use crate::state::{AgentRegistry, AgentSession, AgentStatus};

#[component]
pub fn Sidebar(
    agents: ReadSignal<AgentRegistry>,
    selected_id: Option<String>,
    on_new_agent: EventHandler<()>,
    on_select_agent: EventHandler<String>,
    on_settings: EventHandler<()>,
    on_terminate: EventHandler<String>,
) -> Element {
    let registry = agents.read();

    rsx! {
        div {
            class: "w-72 bg-bg-secondary h-full flex flex-col border-r border-border-default",

            // Header with title and settings gear
            div {
                class: "p-4 border-b border-border-subtle flex items-center justify-between",
                h1 {
                    class: "text-sm font-semibold text-white tracking-tight",
                    "Aether Agents"
                }
                button {
                    class: "text-gray-400 hover:text-white transition-colors p-1 rounded-lg hover:bg-white/10 hover:shadow-md",
                    "data-testid": "settings-button",
                    onclick: move |_| on_settings.call(()),
                    title: "Settings",
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "20",
                        height: "20",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        // Gear icon path
                        path {
                            d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
                        }
                        circle {
                            cx: "12",
                            cy: "12",
                            r: "3"
                        }
                    }
                }
            }

            Stack {
                gap: Space::S1,
                p: Space::S2,
                class: "flex-1 overflow-y-auto",
                for agent in registry.iter_ordered() {
                    {
                        let agent_id = agent.id.clone();
                        let is_selected = selected_id.as_ref() == Some(&agent_id);
                        rsx! {
                            AgentListItem {
                                key: "{agent_id}",
                                agent: agent.clone(),
                                is_selected: is_selected,
                                on_select: {
                                    let id = agent_id.clone();
                                    move |_| on_select_agent.call(id.clone())
                                },
                                on_terminate: {
                                    let id = agent_id.clone();
                                    move |_| on_terminate.call(id.clone())
                                },
                            }
                        }
                    }
                }

                if registry.is_empty() {
                    div {
                        class: "p-4 text-gray-500 text-sm text-center",
                        "data-testid": "no-agents-message",
                        "No agents yet. Create one to get started."
                    }
                }
            }

            // New agent button at bottom
            div {
                class: "p-4 border-t border-border-subtle",
                button {
                    class: "w-full bg-gradient-to-r from-green-500 to-green-600 hover:from-green-400 hover:to-green-500 text-black font-semibold h-10 px-4 rounded-xl flex items-center justify-center gap-2 transition-all hover:shadow-lg hover:scale-[1.02] active:scale-[0.98]",
                    "data-testid": "new-agent-button",
                    onclick: move |_| on_new_agent.call(()),
                    span { class: "text-lg", "+" }
                    span { "New Agent" }
                }
            }
        }
    }
}

#[component]
fn AgentListItem(
    agent: AgentSession,
    is_selected: bool,
    on_select: EventHandler<()>,
    on_terminate: EventHandler<String>,
) -> Element {
    let status_color_class = match &agent.status {
        AgentStatus::Idle => "bg-gray-500",
        AgentStatus::Starting(_) => "bg-yellow-500",
        AgentStatus::Running => "bg-green-500",
        AgentStatus::Error(_) => "bg-red-500",
    };

    let status_class = format!(
        "status-dot w-2 h-2 rounded-full {} {}",
        status_color_class,
        match &agent.status {
            AgentStatus::Running => "status-dot-running",
            AgentStatus::Starting(_) => "animate-pulse",
            _ => "status-dot-idle",
        }
    );

    let selected_class = if is_selected {
        "sidebar-item-selected"
    } else {
        "sidebar-item hover:bg-white/5"
    };

    let agent_id = agent.id.clone();
    let has_context_usage = agent.context_limit > 0;
    let context_usage = agent.context_usage;

    let testid = format!("agent-item-{}", agent_id);
    rsx! {
        div {
            class: "group p-3 cursor-pointer transition-all duration-200 rounded-lg {selected_class}",
            "data-testid": "{testid}",
            onclick: move |_| on_select.call(()),

            Inline {
                gap: Space::S3,
                // Status indicator
                div { class: "{status_class}" }
                // Agent name
                div {
                    class: "font-medium text-gray-100 truncate flex-1 text-sm",
                    "{agent.name}"
                }
                // Terminate button (visible on hover)
                button {
                    class: "opacity-0 group-hover:opacity-100 text-gray-500 hover:text-red-400 transition-all p-1 rounded hover:bg-white/10",
                    onclick: move |e| {
                        e.stop_propagation();
                        on_terminate.call(agent_id.clone());
                    },
                    title: "Terminate agent",
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "14",
                        height: "14",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        line { x1: "18", y1: "6", x2: "6", y2: "18" }
                        line { x1: "6", y1: "6", x2: "18", y2: "18" }
                    }
                }
            }

            // First message preview
            div {
                class: "text-xs text-gray-500 mt-1 truncate ml-5",
                {agent.first_user_message().map(|m| truncate(m, 50)).unwrap_or_default()}
            }

            // Message count
            div {
                class: "text-xs text-gray-600 mt-1 ml-5",
                "{agent.messages.len()} messages"
            }

            if has_context_usage {
                div {
                    class: "mt-2 ml-5",
                    ContextProgressBar { usage: context_usage }
                }
            }
        }
    }
}

/// Truncate a string to the specified length, adding "..." if truncated.
fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated.trim_end())
    }
}

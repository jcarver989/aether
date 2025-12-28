use dioxus::prelude::*;

use crate::state::{AgentRegistry, AgentSession, AgentStatus};

#[component]
pub fn Sidebar(
    agents: ReadSignal<AgentRegistry>,
    selected_id: Option<String>,
    on_new_agent: EventHandler<()>,
    on_select_agent: EventHandler<String>,
    on_settings: EventHandler<()>,
) -> Element {
    let registry = agents.read();

    rsx! {
        div {
            class: "w-72 bg-[#1a1d23] h-full flex flex-col border-r border-[#373b47]",

            // Header with title and settings gear
            div {
                class: "p-4 border-b border-[#2d313a] flex items-center justify-between",
                h1 {
                    class: "text-lg font-semibold text-white tracking-tight",
                    "Aether Agents"
                }
                button {
                    class: "text-gray-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-white/10 hover:shadow-md",
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

            div {
                class: "flex-1 overflow-y-auto space-y-1 p-2",
                for agent_signal in registry.iter_ordered() {
                    {
                        let agent = agent_signal.read();
                        let agent_id = agent.id.clone();
                        let is_selected = selected_id.as_ref() == Some(&agent_id);
                        rsx! {
                            AgentListItem {
                                key: "{agent_id}",
                                agent: agent_signal,
                                is_selected: is_selected,
                                on_select: {
                                    let id = agent_id.clone();
                                    move |_| on_select_agent.call(id.clone())
                                },
                            }
                        }
                    }
                }

                if registry.is_empty() {
                    div {
                        class: "p-4 text-gray-500 text-sm text-center",
                        "No agents yet. Create one to get started."
                    }
                }
            }

            // New agent button at bottom
            div {
                class: "p-4 border-t border-[#2d313a]",
                button {
                    class: "w-full bg-gradient-to-r from-blue-600 to-blue-700 hover:from-blue-500 hover:to-blue-600 text-white font-semibold py-2.5 px-4 rounded-xl flex items-center justify-center gap-2 transition-all hover:shadow-lg hover:scale-[1.02] active:scale-[0.98]",
                    onclick: move |_| on_new_agent.call(()),
                    span { class: "text-xl", "+" }
                    span { "New Agent" }
                }
            }
        }
    }
}

#[component]
fn AgentListItem(
    agent: Signal<AgentSession>,
    is_selected: bool,
    on_select: EventHandler<()>,
) -> Element {
    let agent = agent.read();

    let status_color_class = match &agent.status {
        AgentStatus::Idle => "bg-gray-500",
        AgentStatus::Running => "bg-green-500",
        AgentStatus::Error(_) => "bg-red-500",
    };

    let status_class = format!(
        "status-dot w-2.5 h-2.5 rounded-full {} {}",
        status_color_class,
        if matches!(agent.status, AgentStatus::Running) {
            "status-dot-running"
        } else {
            "status-dot-idle"
        }
    );

    let selected_class = if is_selected {
        "sidebar-item-selected"
    } else {
        "sidebar-item hover:bg-white/5"
    };

    rsx! {
        div {
            class: "p-3 cursor-pointer transition-all duration-200 rounded-lg {selected_class}",
            onclick: move |_| on_select.call(()),

            div {
                class: "flex items-center gap-3",
                // Status indicator
                div { class: "{status_class}" }
                // Agent name
                div {
                    class: "font-medium text-gray-100 truncate flex-1 text-sm",
                    "{agent.name}"
                }
            }

            // First message preview
            div {
                class: "text-xs text-gray-500 mt-1 truncate ml-5",
                {agent.first_user_message().map(|m| truncate(m, 50)).unwrap_or_default()}
            }

            // Message count
            div {
                class: "text-xs text-gray-600 mt-0.5 ml-5",
                "{agent.messages.len()} messages"
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

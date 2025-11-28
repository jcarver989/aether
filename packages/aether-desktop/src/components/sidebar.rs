use dioxus::prelude::*;

use crate::state::{AgentSession, AgentStatus};

#[component]
pub fn Sidebar(
    agents: ReadSignal<Vec<AgentSession>>,
    selected_id: Option<String>,
    on_new_agent: EventHandler<()>,
    on_select_agent: EventHandler<String>,
    on_settings: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "w-72 bg-gray-900 h-full flex flex-col border-r border-gray-800",

            // Header with title and settings gear
            div {
                class: "p-4 border-b border-gray-800 flex items-center justify-between",
                h1 {
                    class: "text-lg font-semibold text-white",
                    "Aether Agents"
                }
                button {
                    class: "text-gray-400 hover:text-white transition-colors p-1 rounded hover:bg-gray-800",
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

            // Agent list
            div {
                class: "flex-1 overflow-y-auto",
                for agent in agents.read().iter() {
                    AgentListItem {
                        key: "{agent.id}",
                        agent: agent.clone(),
                        is_selected: selected_id.as_ref() == Some(&agent.id),
                        on_select: {
                            let id = agent.id.clone();
                            move |_| on_select_agent.call(id.clone())
                        },
                    }
                }

                if agents.read().is_empty() {
                    div {
                        class: "p-4 text-gray-500 text-sm text-center",
                        "No agents yet. Create one to get started."
                    }
                }
            }

            // New agent button at bottom
            div {
                class: "p-4 border-t border-gray-800",
                button {
                    class: "w-full bg-blue-600 hover:bg-blue-700 text-white font-semibold py-2.5 px-4 rounded-lg flex items-center justify-center gap-2 transition-colors",
                    onclick: move |_| on_new_agent.call(()),
                    span { class: "text-xl", "+" }
                    span { "New Agent" }
                }
            }
        }
    }
}

#[component]
fn AgentListItem(agent: AgentSession, is_selected: bool, on_select: EventHandler<()>) -> Element {
    let status_color = match &agent.status {
        AgentStatus::Idle => "bg-gray-500",
        AgentStatus::Running => "bg-green-500 animate-pulse",
        AgentStatus::Error(_) => "bg-red-500",
    };

    let selected_class = if is_selected {
        "bg-gray-800 border-l-4 border-blue-500"
    } else {
        "border-l-4 border-transparent hover:bg-gray-800/50"
    };

    rsx! {
        div {
            class: "p-3 cursor-pointer transition-all duration-150 {selected_class}",
            onclick: move |_| on_select.call(()),

            div {
                class: "flex items-center gap-2",
                // Status indicator
                div { class: "w-2 h-2 rounded-full {status_color}" }
                // Agent name
                div { class: "font-medium text-gray-200 truncate flex-1", "{agent.name}" }
            }

            // Agent command
            div {
                class: "text-xs text-gray-500 mt-1 truncate font-mono",
                "{agent.config.command_line}"
            }

            // Message count
            div {
                class: "text-xs text-gray-600 mt-0.5",
                "{agent.messages.len()} messages"
            }
        }
    }
}

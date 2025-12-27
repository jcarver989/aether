//! View tabs component for switching between Chat and Diff views.

use dioxus::prelude::*;

/// The active view tab for an agent session.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AgentViewTab {
    #[default]
    Chat,
    Diff,
}

/// Tab navigation component for switching between Chat and Diff views.
#[component]
pub fn ViewTabs(active: AgentViewTab, on_change: EventHandler<AgentViewTab>) -> Element {
    let chat_class = if active == AgentViewTab::Chat {
        "px-4 py-2 text-sm font-medium rounded-lg bg-[#252830] text-white"
    } else {
        "px-4 py-2 text-sm font-medium rounded-lg text-gray-400 hover:text-white hover:bg-white/5 transition-colors"
    };

    let diff_class = if active == AgentViewTab::Diff {
        "px-4 py-2 text-sm font-medium rounded-lg bg-[#252830] text-white"
    } else {
        "px-4 py-2 text-sm font-medium rounded-lg text-gray-400 hover:text-white hover:bg-white/5 transition-colors"
    };

    rsx! {
        div {
            class: "flex gap-1 p-1 bg-[#1a1d23] rounded-xl",

            button {
                class: "{chat_class}",
                onclick: move |_| on_change.call(AgentViewTab::Chat),
                "Chat"
            }

            button {
                class: "{diff_class}",
                onclick: move |_| on_change.call(AgentViewTab::Diff),
                "Diff"
            }
        }
    }
}

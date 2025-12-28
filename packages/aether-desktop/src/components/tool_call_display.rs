use crate::state::ToolCallStatus;
use dioxus::prelude::*;

#[component]
pub fn ToolCallDisplay(
    tool_name: String,
    input: String,
    status: ToolCallStatus,
    result: Option<String>,
) -> Element {
    let mut expanded = use_signal(|| false);

    let (icon, icon_color) = match status {
        ToolCallStatus::Pending => ("○", "text-blue-400"),
        ToolCallStatus::Completed => ("✓", "text-green-400"),
        ToolCallStatus::Failed => ("✕", "text-red-400"),
    };

    // Content to display in expandable area
    let display_content = match (&status, &result) {
        (ToolCallStatus::Pending, _) => input.clone(),
        (_, Some(r)) => r.clone(),
        (_, None) => input.clone(),
    };

    rsx! {
        div {
            class: "font-mono text-sm",

            // Header - single line, minimal
            button {
                class: "flex items-center gap-1.5 w-full text-left py-0.5 hover:bg-white/5 rounded transition-colors",
                onclick: move |_| {
                    let current = *expanded.read();
                    expanded.set(!current);
                },

                span {
                    class: "text-gray-500 text-xs w-3",
                    if *expanded.read() { "▼" } else { "▶" }
                }
                span {
                    class: "{icon_color}",
                    "{icon}"
                }
                span {
                    class: "text-gray-400",
                    "{tool_name}"
                }
            }

            // Expandable content - minimal padding
            if *expanded.read() {
                div {
                    class: "ml-5 mt-1 mb-1",
                    pre {
                        class: "text-xs text-gray-400 whitespace-pre-wrap overflow-x-auto max-h-48 overflow-y-auto bg-black/30 rounded px-2 py-1.5",
                        "{display_content}"
                    }
                }
            }
        }
    }
}

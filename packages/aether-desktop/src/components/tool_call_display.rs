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

    let (icon, header_color, label) = match status {
        ToolCallStatus::Pending => (">", "text-blue-400", "Calling"),
        ToolCallStatus::Completed => ("*", "text-green-400", "Result"),
        ToolCallStatus::Failed => ("!", "text-red-400", "Error"),
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

            // Header (always visible)
            button {
                class: "flex items-center gap-2 w-full text-left {header_color} hover:opacity-80 transition-opacity",
                onclick: move |_| {
                    let current = *expanded.read();
                    expanded.set(!current);
                },

                span {
                    class: "transform transition-transform text-xs",
                    if *expanded.read() { "v" } else { ">" }
                }
                span { class: "font-semibold", "{icon}" }
                span { class: "text-gray-400", "{label}:" }
                span { class: "truncate", "{tool_name}" }
            }

            // Expandable content
            if *expanded.read() {
                div {
                    class: "mt-2 pl-4 border-l-2 border-gray-700",
                    pre {
                        class: "text-xs text-gray-400 whitespace-pre-wrap overflow-x-auto max-h-64 overflow-y-auto",
                        "{display_content}"
                    }
                }
            }
        }
    }
}

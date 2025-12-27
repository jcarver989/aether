use crate::state::ToolCallStatus;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn ToolCallDisplay(
    tool_name: String,
    input: String,
    status: ToolCallStatus,
    result: Option<String>,
) -> Element {
    let mut expanded = use_signal(|| false);

    let (icon, header_color, label, bg_color) = match status {
        ToolCallStatus::Pending => (
            ">",
            "text-blue-400",
            "Calling",
            "bg-blue-500/10 border-blue-500/20",
        ),
        ToolCallStatus::Completed => (
            "✓",
            "text-green-400",
            "Result",
            "bg-green-500/10 border-green-500/20",
        ),
        ToolCallStatus::Failed => (
            "✕",
            "text-red-400",
            "Error",
            "bg-red-500/10 border-red-500/20",
        ),
    };

    // Content to display in expandable area
    let display_content = match (&status, &result) {
        (ToolCallStatus::Pending, _) => input.clone(),
        (_, Some(r)) => r.clone(),
        (_, None) => input.clone(),
    };

    rsx! {
        div {
            class: "font-mono text-sm rounded-lg {bg_color} border transition-all duration-200",

            // Header (always visible)
            button {
                class: "tool-call-header flex items-center gap-2 w-full text-left p-3 hover:opacity-90",
                onclick: move |_| {
                    let current = *expanded.read();
                    expanded.set(!current);
                },

                span {
                    class: "transform transition-transform text-xs text-gray-400",
                    if *expanded.read() { "▼" } else { "▶" }
                }
                span {
                    class: "font-semibold text-base {header_color}",
                    "{icon}"
                }
                span {
                    class: "text-gray-400 text-sm",
                    "{label}:"
                }
                span {
                    class: "truncate text-gray-300 font-medium",
                    "{tool_name}"
                }
                // Truncated arguments preview
                if !input.is_empty() {
                    span {
                        class: "text-gray-500 truncate ml-2 text-xs",
                        "{truncate_preview(&input, 50)}"
                    }
                }
            }

            // Expandable content
            if *expanded.read() {
                div {
                    class: "px-3 pb-3 border-t border-white/5",
                    pre {
                        class: "text-xs text-gray-300 whitespace-pre-wrap overflow-x-auto max-h-64 overflow-y-auto bg-black/20 rounded p-3",
                        "{display_content}"
                    }
                }
            }
        }
    }
}

/// Format JSON input as key=value pairs for preview
fn format_input_preview(input: &str) -> String {
    serde_json::from_str::<Value>(input)
        .ok()
        .and_then(|json| json.as_object().cloned())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| {
                    let val = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    format!("{}={}", k, val)
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| input.to_string())
}

/// Truncate preview with ellipsis
fn truncate_preview(input: &str, max_len: usize) -> String {
    let preview = format_input_preview(input);
    if preview.len() <= max_len {
        preview
    } else {
        format!("{}...", &preview[..max_len])
    }
}

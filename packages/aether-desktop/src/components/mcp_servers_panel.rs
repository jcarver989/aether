use crate::EventChannel;
use crate::events::McpEvent;
use crate::state::McpServerStatus;
use dioxus::prelude::*;

/// Displays the status of all configured MCP servers.
#[component]
pub fn McpServersPanel() -> Element {
    let statuses = crate::MCP_SERVER_STATUSES.read();

    rsx! {
        div {
            class: "mcp-servers-panel bg-bg-secondary rounded-lg border border-border-subtle overflow-hidden",
            if statuses.is_empty() {
                div {
                    class: "p-4 text-center text-gray-500 text-sm",
                    "No MCP servers configured."
                }
            } else {
                div {
                    class: "divide-y divide-border-subtle",
                    for (name, status) in statuses.iter() {
                        McpServerRow { key: "{name}", name: name.clone(), status: status.clone() }
                    }
                }
            }
        }
    }
}

/// A single row displaying an MCP server's status.
#[component]
fn McpServerRow(name: String, status: McpServerStatus) -> Element {
    let (indicator_class, status_element) = match &status {
        McpServerStatus::Connected => (
            "bg-green-500",
            rsx! { span { class: "text-xs text-green-400", "Connected" } },
        ),
        McpServerStatus::Connecting => (
            "bg-yellow-500 animate-pulse",
            rsx! { span { class: "text-xs text-yellow-400", "Connecting..." } },
        ),
        McpServerStatus::NeedsOAuth {
            server_id,
            base_url,
        } => (
            "bg-orange-500",
            rsx! { OAuthButton { server_id: server_id.clone(), base_url: base_url.clone() } },
        ),
        McpServerStatus::Failed { error } => (
            "bg-red-500",
            rsx! {
                span {
                    class: "text-xs text-red-400 truncate max-w-32",
                    title: "{error}",
                    "{error}"
                }
            },
        ),
    };

    rsx! {
        div {
            class: "flex items-center gap-3 p-3 hover:bg-white/5 transition-colors",
            div { class: "w-2 h-2 rounded-full {indicator_class}" }
            span { class: "text-sm text-gray-200 flex-1 truncate", "{name}" }
            {status_element}
        }
    }
}

/// Button to trigger OAuth authentication for an MCP server.
#[component]
fn OAuthButton(server_id: String, base_url: String) -> Element {
    let event_channel: EventChannel = use_context();
    let event_tx = event_channel.0;

    let onclick = move |_| {
        let event = McpEvent::StartOAuthFlow {
            server_name: server_id.clone(),
            base_url: base_url.clone(),
        };
        let _ = event_tx.send(event.into());
    };

    rsx! {
        button {
            class: "px-3 py-1.5 text-xs font-medium bg-blue-600 hover:bg-blue-500 text-white rounded-md transition-colors",
            onclick,
            "Authenticate"
        }
    }
}

use crate::components::tool_display::truncate_str;
use crate::components::tool_display::types::SpawnSubAgentDisplayMeta;
use crate::state::{SubAgentStreamingState, SubAgentStreams};
use agent_events::AgentMessage;
use dioxus::prelude::*;

/// Shared component for rendering a list of agent messages.
///
/// Renders accumulated text content and tool events (calls, results, errors).
/// Used by both `SubAgentStreamContent` and `SubAgentStreamInline`.
#[component]
pub fn AgentMessageList(
    messages: Vec<AgentMessage>,
    is_complete: bool,
    /// Extra margin class to apply to content items (e.g., "ml-4")
    #[props(default = "".to_string())]
    content_margin: String,
    /// Test ID prefix for data-testid attributes
    #[props(default = "agent-message".to_string())]
    testid_prefix: String,
) -> Element {
    let text_content: String = messages
        .iter()
        .filter_map(|msg| match msg {
            AgentMessage::Text { chunk, .. } => Some(chunk.as_str()),
            _ => None,
        })
        .collect();

    let margin_class = if content_margin.is_empty() {
        "".to_string()
    } else {
        format!(" {}", content_margin)
    };

    rsx! {
        if !text_content.is_empty() {
            div {
                class: "text-xs text-gray-400 whitespace-pre-wrap{margin_class}",
                "data-testid": "{testid_prefix}-text",
                "{text_content}"
                if !is_complete {
                    span { class: "inline-block w-1.5 h-3 ml-0.5 bg-blue-500 animate-pulse" }
                }
            }
        }

        for (idx, msg) in messages.iter().enumerate() {
            {render_agent_message(msg, idx, &testid_prefix, &margin_class)}
        }
    }
}

fn render_agent_message(
    msg: &AgentMessage,
    idx: usize,
    testid_prefix: &str,
    margin_class: &str,
) -> Option<Element> {
    match msg {
        AgentMessage::ToolCall { request, .. } => {
            let input_summary = truncate_str(&request.arguments, 100);
            Some(rsx! {
                div {
                    class: "flex items-center gap-2 text-[11px]{margin_class}",
                    "data-testid": "{testid_prefix}-tool-call-{idx}",
                    span { class: "text-blue-400 font-mono", "⚒ {request.name}" }
                    span { class: "text-gray-500 italic truncate", "{input_summary}" }
                }
            })
        }
        AgentMessage::ToolResult { result, .. } => {
            let output_summary = truncate_str(&result.result, 100);
            Some(rsx! {
                div {
                    class: "flex items-center gap-2 text-[11px]{margin_class}",
                    "data-testid": "{testid_prefix}-tool-result-{idx}",
                    span { class: "text-green-500", "✓" }
                    span { class: "text-gray-500 font-mono", "{result.name}" }
                    span { class: "text-gray-600 truncate", "{output_summary}" }
                }
            })
        }
        AgentMessage::ToolError { error, .. } => Some(rsx! {
            div {
                class: "flex items-center gap-2 text-[11px]{margin_class}",
                "data-testid": "{testid_prefix}-tool-error-{idx}",
                span { class: "text-red-500", "✗" }
                span { class: "text-red-400 font-mono", "{error.name}" }
                span { class: "text-red-500/70 truncate", "{error.error}" }
            }
        }),
        AgentMessage::Error { message } | AgentMessage::Cancelled { message } => Some(rsx! {
            div {
                class: "text-[11px] text-red-400 bg-red-900/20 px-1 py-0.5 rounded{margin_class}",
                "data-testid": "{testid_prefix}-error-{idx}",
                "Error: {message}"
            }
        }),
        _ => None,
    }
}

/// Display a sub-agent execution with streaming progress.
///
/// This component shows:
/// - Which sub-agent is running (agent name)
/// - What task it's working on (prompt)
/// - Number of tasks in the batch
/// - Current task index (for multi-task operations)
#[component]
pub fn SubAgentDisplay(
    sub_agent_meta: SpawnSubAgentDisplayMeta,
    streams: Option<SubAgentStreams>,
) -> Element {
    let task_label = if sub_agent_meta.task_count > 1 {
        format!(
            "task {} of {}",
            sub_agent_meta.task_index, sub_agent_meta.task_count
        )
    } else {
        "executing".to_string()
    };

    // Truncate prompt if too long
    let prompt_display = if sub_agent_meta.prompt.len() > 200 {
        format!("{}...", &sub_agent_meta.prompt[..200])
    } else {
        sub_agent_meta.prompt.clone()
    };

    rsx! {
        div {
            class: "flex flex-col gap-1 py-1 max-h-64 overflow-y-auto",
            "data-testid": "sub-agent-display",

            div {
                class: "flex items-center gap-2",
                "data-testid": "sub-agent-header",
                span { class: "text-gray-500 text-xs", "→" }
                span {
                    class: "text-blue-400 font-mono text-sm",
                    "data-testid": "sub-agent-name",
                    "{sub_agent_meta.agent_name}"
                }
                if sub_agent_meta.task_count > 1 {
                    span { class: "text-gray-500 text-xs", "({task_label})" }
                }
            }

            div {
                class: "flex items-center gap-2",
                span { class: "text-gray-500 text-xs", " " }
                span {
                    class: "text-sm text-gray-300",
                    "data-testid": "sub-agent-prompt",
                    "{prompt_display}"
                }
            }

            if let Some(streams) = streams {
                for (id, stream) in streams.streams.iter() {
                    SubAgentStreamContent {
                        key: "{id}",
                        stream_id: id.clone(),
                        state: stream.clone()
                    }
                }
            }
        }
    }
}

#[component]
fn SubAgentStreamContent(stream_id: String, state: SubAgentStreamingState) -> Element {
    rsx! {
        div {
            class: "flex flex-col gap-1 ml-4 mt-1 border-l-2 border-gray-700 pl-3",
            "data-testid": "sub-agent-stream-{stream_id}",
            AgentMessageList {
                messages: state.messages.clone(),
                is_complete: state.is_complete,
                testid_prefix: "sub-agent-stream".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_agent_display_single_task() {
        let meta = SpawnSubAgentDisplayMeta {
            agent_name: "codebase-explorer".to_string(),
            prompt: "Explore the codebase to find relevant files".to_string(),
            task_count: 1,
            task_index: 1,
        };

        assert_eq!(meta.task_count, 1);
        assert_eq!(meta.agent_name, "codebase-explorer");
    }

    #[test]
    fn test_sub_agent_display_multi_task() {
        let meta = SpawnSubAgentDisplayMeta {
            agent_name: "codebase-explorer".to_string(),
            prompt: "Explore the codebase".to_string(),
            task_count: 3,
            task_index: 2,
        };

        assert_eq!(meta.task_count, 3);
        assert_eq!(meta.task_index, 2);
    }

    #[test]
    fn test_text_accumulation() {
        use crate::state::SubAgentStreamingState;

        // Text chunks are accumulated by collecting all Text messages
        let state = SubAgentStreamingState {
            agent_name: "test-agent".to_string(),
            messages: vec![
                AgentMessage::Text {
                    message_id: "1".to_string(),
                    chunk: "Hello ".to_string(),
                    is_complete: false,
                    model_name: "test".to_string(),
                },
                AgentMessage::Text {
                    message_id: "2".to_string(),
                    chunk: "world".to_string(),
                    is_complete: true,
                    model_name: "test".to_string(),
                },
            ],
            is_complete: false,
        };
        let text: String = state
            .messages
            .iter()
            .filter_map(|msg| match msg {
                AgentMessage::Text { chunk, .. } => Some(chunk.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "Hello world");
    }
}

use crate::components::tool_display::types::SpawnSubAgentDisplayMeta;
use crate::state::{SubAgentStreamingState, SubAgentStreams};
use agent_events::AgentMessage;
use dioxus::prelude::*;

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
    // Collect text from Text messages (complete ones take precedence)
    let text_content: String = state
        .messages
        .iter()
        .filter_map(|msg| match msg {
            AgentMessage::Text { chunk, .. } => Some(chunk.as_str()),
            _ => None,
        })
        .collect();

    // Count tool events for unique test IDs
    let mut tool_started_idx = 0;
    let mut tool_completed_idx = 0;
    let mut tool_failed_idx = 0;
    let mut error_idx = 0;

    rsx! {
        div {
            class: "flex flex-col gap-1 ml-4 mt-1 border-l-2 border-gray-700 pl-3",
            "data-testid": "sub-agent-stream-{stream_id}",

            // Render accumulated text
            if !text_content.is_empty() {
                div {
                    class: "text-xs text-gray-400 whitespace-pre-wrap",
                    "data-testid": "sub-agent-stream-text",
                    "{text_content}"
                    if !state.is_complete {
                        span { class: "inline-block w-1.5 h-3 ml-0.5 bg-blue-500 animate-pulse" }
                    }
                }
            }

            // Render other stream events (tool calls, errors)
            for msg in &state.messages {
                match msg {
                    AgentMessage::ToolCall { request, .. } => {
                        let idx = tool_started_idx;
                        tool_started_idx += 1;
                        let input_summary = truncate_str(&request.arguments, 100);
                        Some(rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px]",
                                "data-testid": "sub-agent-tool-started-{idx}",
                                span { class: "text-blue-400 font-mono", "⚒ {request.name}" }
                                span { class: "text-gray-500 italic truncate", "{input_summary}" }
                            }
                        })
                    }
                    AgentMessage::ToolResult { result, .. } => {
                        let idx = tool_completed_idx;
                        tool_completed_idx += 1;
                        let output_summary = truncate_str(&result.result, 100);
                        Some(rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px]",
                                "data-testid": "sub-agent-tool-completed-{idx}",
                                span { class: "text-green-500", "✓" }
                                span { class: "text-gray-500 font-mono", "{result.name}" }
                                span { class: "text-gray-600 truncate", "{output_summary}" }
                            }
                        })
                    }
                    AgentMessage::ToolError { error, .. } => {
                        let idx = tool_failed_idx;
                        tool_failed_idx += 1;
                        Some(rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px]",
                                "data-testid": "sub-agent-tool-failed-{idx}",
                                span { class: "text-red-500", "✗" }
                                span { class: "text-red-400 font-mono", "{error.name}" }
                                span { class: "text-red-500/70 truncate", "{error.error}" }
                            }
                        })
                    }
                    AgentMessage::Error { message } | AgentMessage::Cancelled { message } => {
                        let idx = error_idx;
                        error_idx += 1;
                        Some(rsx! {
                            div {
                                class: "text-[11px] text-red-400 bg-red-900/20 px-1 py-0.5 rounded",
                                "data-testid": "sub-agent-error-{idx}",
                                "Error: {message}"
                            }
                        })
                    }
                    _ => None,
                }
            }
        }
    }
}

/// Truncate a string for display, adding "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
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

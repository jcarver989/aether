use crate::components::tool_display::types::{SpawnSubAgentDisplayMeta, SubAgentStreamMessage};
use crate::state::{SubAgentStreamingState, SubAgentStreams};
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
    let text_content = if let Some(full_text) = state.messages.iter().find_map(|msg| match msg {
        SubAgentStreamMessage::TextComplete { full_text } => Some(full_text.as_str()),
        _ => None,
    }) {
        full_text.to_string()
    } else {
        state
            .messages
            .iter()
            .filter_map(|msg| match msg {
                SubAgentStreamMessage::Text { chunk } => Some(chunk.as_str()),
                _ => None,
            })
            .collect::<String>()
    };

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
                    SubAgentStreamMessage::ToolStarted { name, input_summary } => {
                        let idx = tool_started_idx;
                        tool_started_idx += 1;
                        Some(rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px]",
                                "data-testid": "sub-agent-tool-started-{idx}",
                                span { class: "text-blue-400 font-mono", "⚒ {name}" }
                                span { class: "text-gray-500 italic truncate", "{input_summary}" }
                            }
                        })
                    }
                    SubAgentStreamMessage::ToolCompleted { name, output_summary } => {
                        let idx = tool_completed_idx;
                        tool_completed_idx += 1;
                        Some(rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px]",
                                "data-testid": "sub-agent-tool-completed-{idx}",
                                span { class: "text-green-500", "✓" }
                                span { class: "text-gray-500 font-mono", "{name}" }
                                span { class: "text-gray-600 truncate", "{output_summary}" }
                            }
                        })
                    }
                    SubAgentStreamMessage::ToolFailed { name, error } => {
                        let idx = tool_failed_idx;
                        tool_failed_idx += 1;
                        Some(rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px]",
                                "data-testid": "sub-agent-tool-failed-{idx}",
                                span { class: "text-red-500", "✗" }
                                span { class: "text-red-400 font-mono", "{name}" }
                                span { class: "text-red-500/70 truncate", "{error}" }
                            }
                        })
                    }
                    SubAgentStreamMessage::Error { message } => {
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
    fn test_text_accumulation_with_complete() {
        use crate::components::tool_display::types::SubAgentStreamMessage;
        use crate::state::SubAgentStreamingState;

        // Case 1: Only chunks
        let state = SubAgentStreamingState {
            agent_name: "test-agent".to_string(),
            messages: vec![
                SubAgentStreamMessage::Text {
                    chunk: "Hello ".to_string(),
                },
                SubAgentStreamMessage::Text {
                    chunk: "world".to_string(),
                },
            ],
            is_complete: false,
        };
        let text = if let Some(full_text) = state.messages.iter().find_map(|msg| match msg {
            SubAgentStreamMessage::TextComplete { full_text } => Some(full_text.as_str()),
            _ => None,
        }) {
            full_text.to_string()
        } else {
            state
                .messages
                .iter()
                .filter_map(|msg| match msg {
                    SubAgentStreamMessage::Text { chunk } => Some(chunk.as_str()),
                    _ => None,
                })
                .collect::<String>()
        };
        assert_eq!(text, "Hello world");

        // Case 2: Chunks followed by TextComplete (Fixed behavior)
        let state = SubAgentStreamingState {
            agent_name: "test-agent".to_string(),
            messages: vec![
                SubAgentStreamMessage::Text {
                    chunk: "Hello ".to_string(),
                },
                SubAgentStreamMessage::TextComplete {
                    full_text: "Hello world!".to_string(),
                },
            ],
            is_complete: true,
        };

        let text = if let Some(full_text) = state.messages.iter().find_map(|msg| match msg {
            SubAgentStreamMessage::TextComplete { full_text } => Some(full_text.as_str()),
            _ => None,
        }) {
            full_text.to_string()
        } else {
            state
                .messages
                .iter()
                .filter_map(|msg| match msg {
                    SubAgentStreamMessage::Text { chunk } => Some(chunk.as_str()),
                    _ => None,
                })
                .collect::<String>()
        };
        assert_eq!(text, "Hello world!");
    }
}

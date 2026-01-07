use crate::components::tool_display::{
    BashDisplay, EditFileDisplay, ReadFileDisplay, SubAgentDisplay, TodoDisplay, ToolDisplayMeta,
    WriteFileDisplay,
};
use crate::state::{SubAgentStreams, ToolCallStatus};
use agent_events::AgentMessage;
use dioxus::prelude::*;

/// Inline display of sub-agent streaming when we don't have full display_meta yet
#[component]
fn SubAgentStreamInline(
    stream_id: String,
    agent_name: String,
    messages: Vec<AgentMessage>,
    is_complete: bool,
) -> Element {
    let text_content: String = messages
        .iter()
        .filter_map(|msg| match msg {
            AgentMessage::Text { chunk, .. } => Some(chunk.as_str()),
            _ => None,
        })
        .collect();

    rsx! {
        div {
            class: "flex flex-col gap-1 border-l-2 border-blue-600/30 pl-3 py-1 max-h-64 overflow-y-auto",
            "data-testid": "sub-agent-stream-inline-{stream_id}",

            div {
                class: "flex items-center gap-2",
                span { class: "text-gray-500 text-xs", "→" }
                span { class: "text-blue-400 font-mono text-sm", "{agent_name}" }
                if is_complete {
                    span { class: "text-green-500 text-xs", "✓" }
                } else {
                    span { class: "inline-block w-1.5 h-1.5 bg-blue-500 rounded-full animate-pulse" }
                }
            }

            if !text_content.is_empty() {
                div {
                    class: "text-xs text-gray-400 whitespace-pre-wrap ml-4",
                    "{text_content}"
                }
            }

            for msg in &messages {
                match msg {
                    AgentMessage::ToolCall { request, .. } => {
                        let input_summary = truncate_str(&request.arguments, 100);
                        rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px] ml-4",
                                span { class: "text-blue-400 font-mono", "⚒ {request.name}" }
                                span { class: "text-gray-500 italic truncate", "{input_summary}" }
                            }
                        }
                    }
                    AgentMessage::ToolResult { result, .. } => {
                        let output_summary = truncate_str(&result.result, 100);
                        rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px] ml-4",
                                span { class: "text-green-500", "✓" }
                                span { class: "text-gray-500 font-mono", "{result.name}" }
                                span { class: "text-gray-600 truncate", "{output_summary}" }
                            }
                        }
                    }
                    AgentMessage::ToolError { error, .. } => {
                        rsx! {
                            div {
                                class: "flex items-center gap-2 text-[11px] ml-4",
                                span { class: "text-red-500", "✗" }
                                span { class: "text-red-400 font-mono", "{error.name}" }
                                span { class: "text-red-500/70 truncate", "{error.error}" }
                            }
                        }
                    }
                    AgentMessage::Error { message } | AgentMessage::Cancelled { message } => {
                        rsx! {
                            div {
                                class: "text-[11px] text-red-400 ml-4",
                                "Error: {message}"
                            }
                        }
                    }
                    _ => rsx! {}
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

#[component]
pub fn ToolCallDisplay(
    tool_name: String,
    tool_id: String,
    input: String,
    status: ToolCallStatus,
    result: Option<String>,
    display_meta: Option<ToolDisplayMeta>,
    sub_agent_streams: Option<SubAgentStreams>,
) -> Element {
    let (icon, icon_color) = match status {
        ToolCallStatus::Pending => ("○", "text-yellow-400"),
        ToolCallStatus::Completed => ("✓", "text-green-400"),
        ToolCallStatus::Failed => ("✕", "text-red-400"),
    };

    let display_content = if status == ToolCallStatus::Pending {
        input.as_str()
    } else {
        result.as_deref().unwrap_or(input.as_str())
    };

    let display_tool_name = clean_tool_name(&tool_name);

    let (detail_text, expanded_content) =
        build_tool_display(&status, display_meta.as_ref(), display_content, sub_agent_streams);

    rsx! {
        div {
            class: "font-mono text-sm",

            div {
                class: "flex items-center gap-2 py-1",
                span {
                    class: "{icon_color}",
                    "{icon}"
                }
                span {
                    class: "text-gray-200 font-bold",
                    "{display_tool_name}"
                }
                if let Some(detail) = &detail_text {
                    span {
                        class: "text-gray-500",
                        "{detail}"
                    }
                }
            }

            div {
                class: "ml-5 mt-1 mb-1",
                {expanded_content}
            }
        }
    }
}

fn clean_tool_name(tool_name: &str) -> &str {
    tool_name
        .split_once("__")
        .map(|(_, tool_name)| tool_name)
        .unwrap_or(tool_name)
}

fn build_tool_display(
    status: &ToolCallStatus,
    display_meta: Option<&ToolDisplayMeta>,
    display_content: &str,
    sub_agent_streams: Option<SubAgentStreams>,
) -> (Option<String>, Element) {
    // For sub-agent spawns, always show the SubAgentDisplay (even while pending)
    // so we can display streaming progress
    if let Some(ToolDisplayMeta::SpawnSubAgent(sub_agent)) = display_meta {
        let detail_text = display_meta.and_then(|meta| meta.detail_line());
        return (
            detail_text,
            rsx! { SubAgentDisplay { sub_agent_meta: sub_agent.clone(), streams: sub_agent_streams } },
        );
    }

    // If we have sub-agent streams but no display_meta yet (tool still pending),
    // show the streams directly
    if let Some(ref streams) = sub_agent_streams {
        if !streams.streams.is_empty() {
            return (
                None,
                rsx! {
                    div {
                        class: "flex flex-col gap-1",
                        for (id, stream) in streams.streams.iter() {
                            SubAgentStreamInline {
                                key: "{id}",
                                stream_id: id.clone(),
                                agent_name: stream.agent_name.clone(),
                                messages: stream.messages.clone(),
                                is_complete: stream.is_complete,
                            }
                        }
                    }
                },
            );
        }
    }

    if *status == ToolCallStatus::Pending {
        return (None, render_raw_content(display_content));
    }

    let detail_text = display_meta.and_then(|meta| meta.detail_line());

    let expanded_content = match display_meta {
        Some(ToolDisplayMeta::Command(cmd)) => rsx! { BashDisplay { command_meta: cmd.clone() } },
        Some(ToolDisplayMeta::ReadFile(read)) => {
            rsx! { ReadFileDisplay { read_meta: read.clone() } }
        }
        Some(ToolDisplayMeta::WriteFile(write)) => {
            rsx! { WriteFileDisplay { write_meta: write.clone() } }
        }
        Some(ToolDisplayMeta::EditFile(edit)) => {
            rsx! { EditFileDisplay { edit_meta: edit.clone() } }
        }
        Some(ToolDisplayMeta::Todo(todo)) => rsx! { TodoDisplay { todo_meta: todo.clone() } },
        Some(ToolDisplayMeta::SpawnSubAgent(_)) => {
            unreachable!("SpawnSubAgent handled above")
        }
        _ => render_raw_content(display_content),
    };

    (detail_text, expanded_content)
}

fn render_raw_content(display_content: &str) -> Element {
    rsx! {
        pre {
            class: "text-xs text-gray-400 whitespace-pre-wrap overflow-x-auto max-h-48 overflow-y-auto bg-black/30 rounded px-2 py-1",
            "{display_content}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::tool_display::types::{
        CommandDisplayMeta, TodoDisplayMeta, TodoItemMeta,
    };

    #[test]
    fn test_clean_tool_name_strips_prefixes() {
        assert_eq!(clean_tool_name("coding__read_file"), "read_file");
        assert_eq!(clean_tool_name("tasks__list"), "list");
        assert_eq!(clean_tool_name("bash"), "bash");
        assert_eq!(clean_tool_name("my_server__some_tool"), "some_tool");
        assert_eq!(clean_tool_name("a__b__c"), "b__c");
    }

    #[test]
    fn test_tool_detail_text_pending_ignores_meta() {
        let meta = ToolDisplayMeta::Command(CommandDisplayMeta {
            command: "cargo test".to_string(),
            description: None,
            exit_code: 0,
            killed: None,
        });

        let (detail_text, _) =
            build_tool_display(&ToolCallStatus::Pending, Some(&meta), "input", None);
        assert_eq!(detail_text, None);
    }

    #[test]
    fn test_tool_detail_text_todo_counts() {
        let meta = ToolDisplayMeta::Todo(TodoDisplayMeta {
            items: vec![
                TodoItemMeta {
                    content: "Task 1".to_string(),
                    completed: true,
                    active_form: None,
                },
                TodoItemMeta {
                    content: "Task 2".to_string(),
                    completed: false,
                    active_form: None,
                },
            ],
        });

        let (detail_text, _) =
            build_tool_display(&ToolCallStatus::Completed, Some(&meta), "input", None);
        assert_eq!(detail_text, Some("(1/2)".to_string()));
    }

    #[test]
    fn test_tool_detail_text_todo_empty_hidden() {
        let meta = ToolDisplayMeta::Todo(TodoDisplayMeta { items: vec![] });

        let (detail_text, _) =
            build_tool_display(&ToolCallStatus::Completed, Some(&meta), "input", None);
        assert_eq!(detail_text, None);
    }
}

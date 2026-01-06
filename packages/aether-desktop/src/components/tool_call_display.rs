use crate::components::tool_display::{
    BashDisplay, EditFileDisplay, ReadFileDisplay, SubAgentDisplay, TodoDisplay, ToolDisplayMeta,
    WriteFileDisplay,
};
use crate::state::{SubAgentStreams, ToolCallStatus};
use dioxus::prelude::*;

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
        Some(ToolDisplayMeta::SpawnSubAgent(sub_agent)) => {
            rsx! { SubAgentDisplay { sub_agent_meta: sub_agent.clone(), streams: sub_agent_streams } }
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

use dioxus::prelude::*;

use crate::markdown::Markdown;
use crate::state::{Message, MessageKind, Role};

use super::tool_call_display::ToolCallDisplay;

#[component]
pub fn MessageBubble(message: Message) -> Element {
    let is_user = message.role == Role::User;
    let is_tool = matches!(message.kind, MessageKind::ToolCall { .. });

    let alignment = if is_user {
        "justify-end"
    } else {
        "justify-start"
    };
    let bubble_style = if is_user {
        "bg-blue-600 text-white"
    } else if is_tool {
        "bg-gray-800/50 text-gray-300 border border-gray-700"
    } else {
        "bg-gray-800 text-gray-100"
    };

    let max_width = if is_user { "max-w-xl" } else { "max-w-3xl" };

    rsx! {
        div {
            class: "flex {alignment}",
            div {
                class: "{max_width} rounded-lg p-4 {bubble_style}",

                match &message.kind {
                    MessageKind::Text => {
                        if is_user {
                            rsx! {
                                p { class: "whitespace-pre-wrap", "{message.content}" }
                            }
                        } else {
                            rsx! {
                                Markdown {
                                    content: message.content.clone(),
                                    is_streaming: message.is_streaming,
                                }
                            }
                        }
                    }
                    MessageKind::ToolCall {
                        name,
                        status,
                        result,
                    } => {
                        rsx! {
                            ToolCallDisplay {
                                tool_name: name.clone(),
                                input: message.content.clone(),
                                status: status.clone(),
                                result: result.clone(),
                            }
                        }
                    }
                }
            }
        }
    }
}

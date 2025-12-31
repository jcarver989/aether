use dioxus::prelude::*;

use crate::markdown::Markdown;
use crate::state::{Message, MessageKind, Role};

use super::tool_call_display::ToolCallDisplay;

#[component]
pub fn MessageBubble(message: Message) -> Element {
    let is_user = message.role == Role::User;

    rsx! {
        div {
            class: "py-1",

            match &message.kind {
                MessageKind::Text => {
                    if is_user {
                        rsx! {
                            div {
                                class: "text-green-300/80 text-sm",
                                span {
                                    class: "text-green-500/60 mr-2",
                                    ">"
                                }
                                "{message.content}"
                            }
                        }
                    } else {
                        rsx! {
                            div {
                                class: "text-gray-200",
                                Markdown {
                                    content: message.content.clone(),
                                    is_streaming: message.is_streaming,
                                }
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

use iocraft::prelude::*;
use wisp::colors;

use crate::app_state::ChatMessage;

#[derive(Default, Props)]
pub struct AppViewProps {
    pub show_input: bool,
    pub messages: Option<State<Vec<ChatMessage>>>,
}

#[component]
pub fn ChatMessageList(props: &AppViewProps) -> impl Into<AnyElement<'static>> {
    let Some(messages_state) = props.messages else {
        panic!("Messages required!");
    };

    element! {
        View(
            flex_direction: FlexDirection::Column,
        ) {
            #(
                messages_state.read().iter().enumerate().map(|(_i, message)| {
                    match message {
                        ChatMessage::Assistant { message_id, text, ..} => {
                            element! {
                                View(key: message_id.clone(), border_style: BorderStyle::Round, border_color: colors::primary(), padding: 1) {
                                    Text(key: message_id.clone(), content: text)
                                }
                            }
                        }
                        ChatMessage::User { text, ..}  => {
                            element! {
                                View {
                                    Text(content: text)
                                }
                            }
                        }
                        ChatMessage::ToolCall { id, name, arguments, result, is_complete, .. } => {
                            element! {
                                View(key: id.clone(), border_style: BorderStyle::Round, border_color: colors::info(), padding: 1) {
                                    Text(key: format!("{}-name", id), content: format!("🔧 {}", name), weight: Weight::Bold)
                                    #(if let Some(args) = arguments {
                                        element! {
                                            View {
                                                Text(key: format!("{}-args", id), content: format!("Args: {}", args))
                                            }
                                        }
                                    } else {
                                        element! { View }
                                    })
                                    #(if let Some(res) = result {
                                        element! {
                                            View {
                                                Text(key: format!("{}-result", id), content: format!("Result: {}", res))
                                            }
                                        }
                                    } else if *is_complete {
                                        element! {
                                            View {
                                                Text(key: format!("{}-complete", id), content: "✓ Complete")
                                            }
                                        }
                                    } else {
                                        element! {
                                            View {
                                                Text(key: format!("{}-running", id), content: "⏳ Running...")
                                            }
                                        }
                                    })
                                }
                            }
                        }
                    }
                })
            )
        }
    }
}

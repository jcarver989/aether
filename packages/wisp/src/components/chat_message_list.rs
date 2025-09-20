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
                    }
                })
            )
        }
    }
}

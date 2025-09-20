use crate::{
    app_state::{AppState, ChatMessage},
    components::ChatMessageList,
};
use aether::agent::{AgentMessage, UserMessage};
use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct ScreenProps {}

#[component]
pub fn Screen(_props: &ScreenProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut system = hooks.use_context_mut::<SystemContext>();
    let state = hooks.use_context::<AppState>();
    let tx = state.agent_tx.clone();
    let rx = state.agent_rx.clone();
    let mut input_message = hooks.use_state(|| "".to_string());
    let mut should_exit = hooks.use_state(|| false);
    let mut should_submit = hooks.use_state(|| false);
    let mut messages = hooks.use_state(|| Vec::<ChatMessage>::new());
    let (width, height) = hooks.use_terminal_size();

    hooks.use_future(async move {
        while let Some(message) = rx.lock().await.recv().await {
            match message {
                AgentMessage::Text {
                    message_id,
                    chunk,
                    is_complete: false,
                    ..
                } => {
                    let mut msgs = messages.write();
                    if let Some(index) = msgs.iter().position(|msg| match msg {
                        ChatMessage::Assistant {
                            message_id: existing_id,
                            ..
                        } => existing_id == &message_id,
                        _ => false,
                    }) {
                        match &mut msgs[index] {
                            ChatMessage::Assistant { text, .. } => text.push_str(&chunk),
                            _ => {}
                        }
                    } else {
                        msgs.push(ChatMessage::Assistant {
                            message_id,
                            text: chunk,
                        })
                    }
                }
                _ => {}
            }
        }
    });

    hooks.use_terminal_events(move |event| match event {
        TerminalEvent::Key(KeyEvent { code, kind, .. }) if kind != KeyEventKind::Release => {
            match code {
                KeyCode::Char('q') if input_message.to_string().is_empty() => {
                    should_exit.set(true);
                }
                KeyCode::Enter if !input_message.to_string().trim().is_empty() => {
                    should_submit.set(true);
                }
                _ => {}
            }
        }
        _ => {}
    });

    if should_exit.get() {
        system.exit();
    }

    if should_submit.get() {
        let user_input = input_message.to_string().trim().to_string();
        let tx_clone = tx.clone();

        // Spawn a tokio task to send the message
        tokio::spawn(async move {
            if let Err(e) = tx_clone
                .send(UserMessage::Text {
                    content: user_input,
                })
                .await
            {
                eprintln!("Failed to send message: {}", e);
            }
        });

        input_message.set("".to_string());
        should_submit.set(false);
    }

    element! {
        View(
            width,
            height,
            flex_direction: FlexDirection::Column
        ) {
            View(
                margin: 2,
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::Hidden,
            ) {
                ChatMessageList(
                    show_input: true,
                    messages: messages
                )
            }

            View(
                border_style: BorderStyle::Round,
                border_color: crate::colors::accent(),
                margin: 4
            ) {
                View(flex_direction: FlexDirection::Row, align_items: AlignItems::Center) {
                    Text(
                        content: "> ",
                        color: crate::colors::accent(),
                        weight: Weight::Bold,
                    )
                    View(flex_grow: 1.0) {
                        TextInput(
                            has_focus: true,
                            value: input_message.to_string(),
                            on_change: move |new_value| input_message.set(new_value),
                            multiline: false
                        )
                    }
                }
            }
        }
    }
}

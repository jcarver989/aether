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
    let agent = state.agent.clone();
    let agent_for_send = state.agent.clone();
    let mut input_message = hooks.use_state(|| "".to_string());
    let mut should_exit = hooks.use_state(|| false);
    let mut should_submit = hooks.use_state(|| false);
    let mut messages = hooks.use_state(|| Vec::<ChatMessage>::new());
    let mut scroll_offset = hooks.use_state(|| 0i32);
    let (width, height) = hooks.use_terminal_size();

    hooks.use_future(async move {
        while let Some(message) = agent.lock().await.recv().await {
            let mut msgs = messages.write();
            match message {
                AgentMessage::Text {
                    message_id,
                    chunk,
                    is_complete: false,
                    ..
                } => {
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

                AgentMessage::ToolCall {
                    tool_call_id,
                    name,
                    arguments,
                    result,
                    is_complete,
                    model_name,
                } => {
                    if let Some(index) = msgs.iter().position(|msg| match msg {
                        ChatMessage::ToolCall { id, .. } => id == &tool_call_id,
                        _ => false,
                    }) {
                        // Update existing tool call
                        if let ChatMessage::ToolCall {
                            arguments: existing_args,
                            result: existing_result,
                            is_complete: existing_complete,
                            ..
                        } = &mut msgs[index]
                        {
                            if arguments.is_some() {
                                *existing_args = arguments;
                            }
                            if result.is_some() {
                                *existing_result = result;
                            }
                            *existing_complete = is_complete;
                        }
                    } else {
                        // Create new tool call
                        msgs.push(ChatMessage::ToolCall {
                            id: tool_call_id,
                            name,
                            arguments,
                            result,
                            model_name,
                            is_complete,
                        });
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
                KeyCode::Up => {
                    scroll_offset.set((scroll_offset.get() - 1).max(0));
                }
                KeyCode::Down => {
                    scroll_offset.set(scroll_offset.get() + 1);
                }
                _ => {}
            }
        }
        TerminalEvent::FullscreenMouse(FullscreenMouseEvent { kind, .. }) => match kind {
            MouseEventKind::ScrollUp => {
                scroll_offset.set((scroll_offset.get() - 3).max(0));
            }
            MouseEventKind::ScrollDown => {
                scroll_offset.set(scroll_offset.get() + 3);
            }
            _ => {}
        }
        _ => {}
    });

    if should_exit.get() {
        system.exit();
    }

    if should_submit.get() {
        let user_input = input_message.to_string().trim().to_string();
        let agent_clone = agent_for_send.clone();

        // Spawn a tokio task to send the message
        tokio::spawn(async move {
            if let Err(e) = agent_clone
                .lock()
                .await
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
                overflow: Overflow::Hidden
            ) {
                View(
                    position: Position::Absolute,
                    top: -scroll_offset.get(),
                ) {
                    ChatMessageList(
                        show_input: true,
                        messages: messages
                    )
                }
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
                    View(
                        flex_grow: 1.0,
                        width: width,
                        height: 1,
                        padding: 0
                    ) {
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

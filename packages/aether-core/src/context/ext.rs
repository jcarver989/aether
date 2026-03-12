use crate::events::AgentMessage;
use llm::types::IsoString;
use llm::{AssistantReasoning, ChatMessage, Context, ToolCallError, ToolCallResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UserEvent {
    Message { content: String },
    ClearContext,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum SessionEvent {
    User(UserEvent),
    Agent(AgentMessage),
}

pub trait ContextExt {
    fn from_events(events: &[SessionEvent]) -> Self
    where
        Self: Sized;
}

impl ContextExt for Context {
    fn from_events(events: &[SessionEvent]) -> Self {
        let mut context = Context::new(vec![], vec![]);
        let mut acc = TurnAccumulator::default();
        for event in events {
            match event {
                SessionEvent::User(e) => apply_user_event(&mut context, e),
                SessionEvent::Agent(m) => apply_agent_event(&mut context, m, &mut acc),
            }
        }
        context
    }
}

#[derive(Default)]
struct TurnAccumulator {
    text: String,
    reasoning: String,
    tool_results: Vec<Result<ToolCallResult, ToolCallError>>,
}

fn apply_user_event(ctx: &mut Context, event: &UserEvent) {
    match event {
        UserEvent::Message { content } => {
            ctx.add_message(ChatMessage::User {
                content: content.clone(),
                timestamp: IsoString::now(),
            });
        }
        UserEvent::ClearContext => {
            ctx.clear_conversation();
        }
    }
}

fn apply_agent_event(ctx: &mut Context, event: &AgentMessage, acc: &mut TurnAccumulator) {
    match event {
        AgentMessage::Text {
            chunk,
            is_complete: true,
            ..
        } => {
            acc.text = chunk.clone();
        }
        AgentMessage::Thought {
            chunk,
            is_complete: true,
            ..
        } => {
            acc.reasoning = chunk.clone();
        }
        AgentMessage::ToolResult { result, .. } => {
            acc.tool_results.push(Ok(result.clone()));
        }
        AgentMessage::ToolError { error, .. } => {
            acc.tool_results.push(Err(error.clone()));
        }
        AgentMessage::Done => {
            let text = std::mem::take(&mut acc.text);
            let reasoning_text = std::mem::take(&mut acc.reasoning);
            let tools = std::mem::take(&mut acc.tool_results);
            if !text.is_empty() || !tools.is_empty() {
                let reasoning = AssistantReasoning::from_parts(reasoning_text, None);
                ctx.push_assistant_turn(&text, reasoning, tools);
            }
        }
        AgentMessage::ContextCleared => {
            ctx.clear_conversation();
            acc.text.clear();
            acc.reasoning.clear();
            acc.tool_results.clear();
        }
        AgentMessage::ContextCompactionResult { summary, .. } => {
            *ctx = ctx.with_compacted_summary(summary);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm::ToolCallResult;

    fn system_context() -> Context {
        Context::new(
            vec![ChatMessage::System {
                content: "You are helpful.".to_string(),
                timestamp: IsoString::now(),
            }],
            vec![],
        )
    }

    #[test]
    fn apply_user_message_adds_user_message() {
        let mut ctx = system_context();
        apply_user_event(
            &mut ctx,
            &UserEvent::Message {
                content: "Hello".to_string(),
            },
        );

        assert_eq!(ctx.message_count(), 2);
        assert!(matches!(ctx.messages()[1], ChatMessage::User { .. }));
    }

    #[test]
    fn apply_user_clear_retains_system_messages() {
        let mut ctx = system_context();
        apply_user_event(
            &mut ctx,
            &UserEvent::Message {
                content: "Hello".to_string(),
            },
        );
        apply_user_event(&mut ctx, &UserEvent::ClearContext);

        assert_eq!(ctx.message_count(), 1);
        assert!(ctx.messages()[0].is_system());
    }

    #[test]
    fn apply_agent_produces_assistant_and_tool_results() {
        let mut ctx = system_context();
        let mut acc = TurnAccumulator::default();

        apply_agent_event(
            &mut ctx,
            &AgentMessage::ToolResult {
                result: ToolCallResult {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                    result: "file contents".to_string(),
                },
                result_meta: None,
                model_name: "test".to_string(),
            },
            &mut acc,
        );

        apply_agent_event(
            &mut ctx,
            &AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "Here is the file".to_string(),
                is_complete: true,
                model_name: "test".to_string(),
            },
            &mut acc,
        );

        apply_agent_event(&mut ctx, &AgentMessage::Done, &mut acc);

        assert_eq!(ctx.message_count(), 3);
        match &ctx.messages()[1] {
            ChatMessage::Assistant {
                content,
                tool_calls,
                ..
            } => {
                assert_eq!(content, "Here is the file");
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "read_file");
            }
            other => panic!("Expected Assistant, got {other:?}"),
        }
        assert!(ctx.messages()[2].is_tool_result());
    }

    #[test]
    fn apply_agent_context_cleared() {
        let mut ctx = system_context();
        let mut acc = TurnAccumulator::default();

        apply_user_event(
            &mut ctx,
            &UserEvent::Message {
                content: "Hello".to_string(),
            },
        );
        apply_agent_event(&mut ctx, &AgentMessage::ContextCleared, &mut acc);

        assert_eq!(ctx.message_count(), 1);
        assert!(ctx.messages()[0].is_system());
    }

    #[test]
    fn apply_agent_compaction_replaces_with_summary() {
        let mut ctx = system_context();
        let mut acc = TurnAccumulator::default();

        apply_user_event(
            &mut ctx,
            &UserEvent::Message {
                content: "Hello".to_string(),
            },
        );
        apply_agent_event(
            &mut ctx,
            &AgentMessage::ContextCompactionResult {
                summary: "Summary of conversation".to_string(),
                messages_removed: 1,
            },
            &mut acc,
        );

        assert_eq!(ctx.message_count(), 2);
        assert!(ctx.messages()[0].is_system());
        assert!(ctx.messages()[1].is_summary());
    }

    #[test]
    fn done_without_content_does_not_add_message() {
        let mut ctx = system_context();
        let mut acc = TurnAccumulator::default();

        apply_agent_event(&mut ctx, &AgentMessage::Done, &mut acc);

        assert_eq!(ctx.message_count(), 1);
    }

    #[test]
    fn streaming_chunks_are_ignored() {
        let mut ctx = system_context();
        let mut acc = TurnAccumulator::default();

        apply_agent_event(
            &mut ctx,
            &AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "partial".to_string(),
                is_complete: false,
                model_name: "test".to_string(),
            },
            &mut acc,
        );

        assert_eq!(ctx.message_count(), 1);
    }

    #[test]
    fn accumulator_resets_after_done() {
        let mut ctx = system_context();
        let mut acc = TurnAccumulator::default();

        apply_agent_event(
            &mut ctx,
            &AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "Turn 1".to_string(),
                is_complete: true,
                model_name: "test".to_string(),
            },
            &mut acc,
        );
        apply_agent_event(&mut ctx, &AgentMessage::Done, &mut acc);

        apply_agent_event(
            &mut ctx,
            &AgentMessage::Text {
                message_id: "msg_2".to_string(),
                chunk: "Turn 2".to_string(),
                is_complete: true,
                model_name: "test".to_string(),
            },
            &mut acc,
        );
        apply_agent_event(&mut ctx, &AgentMessage::Done, &mut acc);

        assert_eq!(ctx.message_count(), 3);
    }

    #[test]
    fn user_event_serde_roundtrip() {
        let event = UserEvent::Message {
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);

        let clear = UserEvent::ClearContext;
        let json = serde_json::to_string(&clear).unwrap();
        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, clear);
    }

    #[test]
    fn from_events_basic_conversation() {
        let events = vec![
            SessionEvent::User(UserEvent::Message {
                content: "Hello".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "Hi there!".to_string(),
                is_complete: true,
                model_name: "test".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Done),
        ];

        let ctx = Context::from_events(&events);

        assert_eq!(ctx.message_count(), 2);
        assert!(matches!(ctx.messages()[0], ChatMessage::User { .. }));
        assert!(matches!(ctx.messages()[1], ChatMessage::Assistant { .. }));
    }

    #[test]
    fn from_events_with_tool_calls() {
        let events = vec![
            SessionEvent::User(UserEvent::Message {
                content: "Read Cargo.toml".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::ToolCall {
                request: llm::ToolCallRequest {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                },
                model_name: "test".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::ToolResult {
                result: ToolCallResult {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                    result: "file contents".to_string(),
                },
                result_meta: None,
                model_name: "test".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "Here is the file".to_string(),
                is_complete: true,
                model_name: "test".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Done),
        ];

        let ctx = Context::from_events(&events);

        assert_eq!(ctx.message_count(), 3);
        match &ctx.messages()[1] {
            ChatMessage::Assistant { tool_calls, .. } => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "read_file");
            }
            other => panic!("Expected Assistant, got {other:?}"),
        }
        assert!(ctx.messages()[2].is_tool_result());
    }

    #[test]
    fn from_events_handles_clear() {
        let events = vec![
            SessionEvent::User(UserEvent::Message {
                content: "Hello".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "Hi!".to_string(),
                is_complete: true,
                model_name: "test".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Done),
            SessionEvent::User(UserEvent::ClearContext),
            SessionEvent::User(UserEvent::Message {
                content: "Start fresh".to_string(),
            }),
        ];

        let ctx = Context::from_events(&events);

        assert_eq!(ctx.message_count(), 1);
        assert!(matches!(ctx.messages()[0], ChatMessage::User { .. }));
    }

    #[test]
    fn from_events_handles_compaction() {
        let events = vec![
            SessionEvent::User(UserEvent::Message {
                content: "Hello".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "Hi!".to_string(),
                is_complete: true,
                model_name: "test".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Done),
            SessionEvent::Agent(AgentMessage::ContextCompactionResult {
                summary: "Earlier we greeted each other.".to_string(),
                messages_removed: 2,
            }),
            SessionEvent::User(UserEvent::Message {
                content: "What did we talk about?".to_string(),
            }),
        ];

        let ctx = Context::from_events(&events);

        assert_eq!(ctx.message_count(), 2);
        assert!(ctx.messages()[0].is_summary());
    }

    #[test]
    fn from_events_empty() {
        let ctx = Context::from_events(&[]);
        assert_eq!(ctx.message_count(), 0);
    }
}

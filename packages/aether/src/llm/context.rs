use serde::{Deserialize, Serialize};

use crate::types::IsoString;

use super::{ChatMessage, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    messages: Vec<ChatMessage>,
    tools: Vec<ToolDefinition>,
}

impl Context {
    pub fn new(messages: Vec<ChatMessage>, tools: Vec<ToolDefinition>) -> Self {
        Self { messages, tools }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    pub fn set_tools(&mut self, tools: Vec<ToolDefinition>) {
        self.tools = tools;
    }

    pub fn messages(&self) -> &Vec<ChatMessage> {
        &self.messages
    }

    pub fn tools(&self) -> &Vec<ToolDefinition> {
        &self.tools
    }

    /// Returns the number of messages in the context
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Get all non-system messages for summarization
    pub fn messages_for_summary(&self) -> Vec<&ChatMessage> {
        self.messages
            .iter()
            .filter(|msg| !msg.is_system())
            .collect()
    }

    /// Create a new context with all messages replaced by a summary.
    /// Preserves the system prompt and tools.
    pub fn with_compacted_summary(&self, summary: &str) -> Context {
        let system_messages: Vec<_> = self
            .messages
            .iter()
            .filter(|msg| msg.is_system())
            .cloned()
            .collect();

        let non_system_count = self.messages.len() - system_messages.len();

        let mut messages = system_messages;
        if non_system_count > 0 {
            messages.push(ChatMessage::Summary {
                content: summary.to_string(),
                timestamp: IsoString::now(),
                messages_compacted: non_system_count,
            });
        }

        Context {
            messages,
            tools: self.tools.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ToolCallResult;

    fn create_test_context() -> Context {
        let messages = vec![
            ChatMessage::System {
                content: "You are a helpful assistant.".to_string(),
                timestamp: IsoString::now(),
            },
            ChatMessage::User {
                content: "Hello".to_string(),
                timestamp: IsoString::now(),
            },
            ChatMessage::Assistant {
                content: "Hi there!".to_string(),
                reasoning_content: None,
                timestamp: IsoString::now(),
                tool_calls: vec![],
            },
            ChatMessage::ToolCallResult(Ok(ToolCallResult {
                id: "1".to_string(),
                name: "tool1".to_string(),
                arguments: "{}".to_string(),
                result: "Result 1".to_string(),
            })),
            ChatMessage::ToolCallResult(Ok(ToolCallResult {
                id: "2".to_string(),
                name: "tool2".to_string(),
                arguments: "{}".to_string(),
                result: "Result 2".to_string(),
            })),
            ChatMessage::ToolCallResult(Ok(ToolCallResult {
                id: "3".to_string(),
                name: "tool3".to_string(),
                arguments: "{}".to_string(),
                result: "Result 3".to_string(),
            })),
        ];
        Context::new(messages, vec![])
    }

    #[test]
    fn test_message_count() {
        let ctx = create_test_context();
        assert_eq!(ctx.message_count(), 6);
    }

    #[test]
    fn test_with_compacted_summary_preserves_system_prompt() {
        let ctx = create_test_context();
        let compacted = ctx.with_compacted_summary("This is a summary of previous conversation.");

        assert_eq!(compacted.message_count(), 2);
        assert!(compacted.messages()[0].is_system());
        assert!(compacted.messages()[1].is_summary());
    }

    #[test]
    fn test_with_compacted_summary_empty_context() {
        let ctx = Context::new(
            vec![ChatMessage::System {
                content: "System".to_string(),
                timestamp: IsoString::now(),
            }],
            vec![],
        );
        let compacted = ctx.with_compacted_summary("Summary");

        assert_eq!(compacted.message_count(), 1);
    }

    #[test]
    fn test_messages_for_summary() {
        let ctx = create_test_context();
        let msgs = ctx.messages_for_summary();

        assert_eq!(msgs.len(), 5);
        assert!(msgs.iter().all(|m| !m.is_system()));
    }
}

use serde::{Deserialize, Serialize};

use crate::reasoning::ReasoningEffort;
use crate::types::IsoString;

use super::{ChatMessage, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    messages: Vec<ChatMessage>,
    tools: Vec<ToolDefinition>,
    #[serde(skip)]
    reasoning_effort: Option<ReasoningEffort>,
}

impl Context {
    pub fn new(messages: Vec<ChatMessage>, tools: Vec<ToolDefinition>) -> Self {
        Self {
            messages,
            tools,
            reasoning_effort: None,
        }
    }

    pub fn reasoning_effort(&self) -> Option<ReasoningEffort> {
        self.reasoning_effort
    }

    pub fn set_reasoning_effort(&mut self, effort: Option<ReasoningEffort>) {
        self.reasoning_effort = effort;
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

    /// Estimate total token count using the ~4 bytes/token heuristic.
    /// Includes messages and tool definitions. Used for pre-flight overflow detection.
    pub fn estimated_token_count(&self) -> u32 {
        let message_bytes: usize = self.messages.iter().map(ChatMessage::estimated_bytes).sum();
        let tool_bytes: usize = self
            .tools
            .iter()
            .map(|t| t.name.len() + t.description.len() + t.parameters.len())
            .sum();
        let total_bytes = message_bytes + tool_bytes;
        u32::try_from(total_bytes / 4).unwrap_or(u32::MAX)
    }

    /// Build an assistant turn and its tool call results and append them to messages.
    pub fn push_assistant_turn(
        &mut self,
        content: &str,
        reasoning_content: &str,
        completed_tools: Vec<Result<super::ToolCallResult, super::ToolCallError>>,
    ) {
        let tool_requests: Vec<_> = completed_tools
            .iter()
            .map(|result| match result {
                Ok(r) => super::ToolCallRequest {
                    id: r.id.clone(),
                    name: r.name.clone(),
                    arguments: r.arguments.clone(),
                },
                Err(e) => super::ToolCallRequest {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    arguments: e.arguments.clone().unwrap_or_default(),
                },
            })
            .collect();

        self.messages.push(ChatMessage::Assistant {
            content: content.to_string(),
            reasoning_content: (!reasoning_content.is_empty())
                .then_some(reasoning_content.to_string()),
            timestamp: IsoString::now(),
            tool_calls: tool_requests,
        });

        for result in completed_tools {
            self.messages.push(ChatMessage::ToolCallResult(result));
        }
    }

    /// Clear all non-system messages, retaining only system prompts.
    pub fn clear_conversation(&mut self) {
        self.messages.retain(|msg| msg.is_system());
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
            reasoning_effort: self.reasoning_effort,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolCallResult;

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

    #[test]
    fn test_reasoning_effort_default_is_none() {
        let ctx = create_test_context();
        assert_eq!(ctx.reasoning_effort(), None);
    }

    #[test]
    fn test_reasoning_effort_set_and_get() {
        let mut ctx = create_test_context();
        ctx.set_reasoning_effort(Some(crate::ReasoningEffort::High));
        assert_eq!(ctx.reasoning_effort(), Some(crate::ReasoningEffort::High));

        ctx.set_reasoning_effort(None);
        assert_eq!(ctx.reasoning_effort(), None);
    }

    #[test]
    fn test_reasoning_effort_preserved_through_compaction() {
        let mut ctx = create_test_context();
        ctx.set_reasoning_effort(Some(crate::ReasoningEffort::Medium));
        let compacted = ctx.with_compacted_summary("Summary");
        assert_eq!(
            compacted.reasoning_effort(),
            Some(crate::ReasoningEffort::Medium)
        );
    }

    #[test]
    fn test_estimated_token_count() {
        use crate::ToolDefinition;

        // "You are a helpful assistant." = 28 bytes
        // "Hello" = 5 bytes
        // "Hi there!" = 9 bytes (assistant, no reasoning, no tool calls)
        // 3 tool results: "Result 1" (8) + "tool1" (5) + "{}" (2) = 15 each = 45 total
        // Total message bytes = 28 + 5 + 9 + 45 = 87
        let ctx = create_test_context();
        let base_estimate = ctx.estimated_token_count();

        // With no tools, estimate = message_bytes / 4
        assert_eq!(base_estimate, 87 / 4);

        // Now add a tool definition and verify it increases
        let tool = ToolDefinition {
            name: "read_file".to_string(),           // 9
            description: "Reads a file".to_string(), // 12
            parameters: "{}".to_string(),            // 2
            server: None,
        };
        let ctx_with_tools = Context::new(ctx.messages().clone(), vec![tool]);
        let with_tools_estimate = ctx_with_tools.estimated_token_count();
        assert_eq!(with_tools_estimate, (87 + 9 + 12 + 2) / 4);
        assert!(with_tools_estimate > base_estimate);
    }
}

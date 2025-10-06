use crate::agent::ToolCallResult;
use crate::types::{ChatMessage, IsoString};
use std::collections::HashSet;

/// Encapsulates state for a single agent iteration
/// An iteration consists of one LLM call and any resulting tool executions
pub struct AgenticIterationState {
    /// Current message ID from LLM
    current_message_id: Option<String>,
    /// Accumulated message content
    message_content: String,
    /// Tool call IDs that have been sent but not yet completed
    pending_tools: HashSet<String>,
    /// Tool call results that have been completed
    completed_tools: Vec<ToolCallResult>,
    /// Whether the LLM stream has completed
    llm_done: bool,
}

impl AgenticIterationState {
    pub fn new() -> Self {
        Self {
            current_message_id: None,
            message_content: String::new(),
            pending_tools: HashSet::new(),
            completed_tools: Vec::new(),
            llm_done: false,
        }
    }

    /// Set the message ID for this iteration
    pub fn set_message_id(&mut self, id: String) {
        self.current_message_id = Some(id);
        self.message_content.clear();
    }

    /// Append content to the current message
    pub fn append_content(&mut self, chunk: &str) {
        self.message_content.push_str(chunk);
    }

    /// Get the current message ID
    pub fn current_message_id(&self) -> Option<&str> {
        self.current_message_id.as_deref()
    }

    /// Get final message content if we have a message
    pub fn final_message_content(&self) -> Option<&str> {
        if self.current_message_id.is_some() {
            Some(&self.message_content)
        } else {
            None
        }
    }

    /// Mark a tool as sent (pending execution)
    pub fn mark_tool_sent(&mut self, id: String) {
        self.pending_tools.insert(id);
    }

    /// Mark a tool as complete and store its result
    pub fn mark_tool_complete(&mut self, result: ToolCallResult) {
        self.pending_tools.remove(&result.id);
        self.completed_tools.push(result);
    }

    /// Mark the LLM stream as done
    pub fn mark_llm_done(&mut self) {
        self.llm_done = true;
    }

    /// Check if the LLM stream is done
    pub fn is_llm_done(&self) -> bool {
        self.llm_done
    }

    /// Check if all pending tools are complete
    pub fn all_tools_complete(&self) -> bool {
        self.pending_tools.is_empty()
    }

    /// Check if this iteration produced any tool results
    pub fn has_tool_results(&self) -> bool {
        !self.completed_tools.is_empty()
    }

    /// Determine if the agent loop should continue after this iteration
    /// Returns true if: we have a final message AND we have tool results
    pub fn should_continue_loop(&self) -> bool {
        self.final_message_content().is_some() && self.has_tool_results()
    }

    /// Convert this iteration's state into ChatMessages for the context
    /// Returns messages in the correct order: Assistant message first, then ToolCallResults
    pub fn into_context_messages(self) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // Extract tool requests from completed tools
        let tool_requests: Vec<_> = self
            .completed_tools
            .iter()
            .map(|result| result.request.clone())
            .collect();

        // Add assistant message with tool calls
        messages.push(ChatMessage::Assistant {
            content: self.message_content,
            timestamp: IsoString::now(),
            tool_calls: tool_requests,
        });

        // Add tool results
        for result in self.completed_tools {
            messages.push(ChatMessage::ToolCallResult {
                tool_call_id: result.id,
                content: result.result,
                timestamp: IsoString::now(),
            });
        }

        messages
    }
}

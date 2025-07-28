use crate::{
    llm::{ChatMessage as LlmChatMessage, ChatRequest, LlmProvider, ToolDefinition},
    mcp::registry::ToolRegistry,
    types::ChatMessage,
};
use color_eyre::Result;
use std::collections::{HashMap, VecDeque};

/// Represents an AI agent with its associated LLM provider, conversation context, and tools
pub struct Agent<T: LlmProvider> {
    /// The LLM provider (e.g., OpenRouter, Ollama) for this agent
    llm_provider: T,

    /// Tool registry containing available tools and MCP clients
    tool_registry: ToolRegistry,

    /// Conversation history for this agent
    conversation_history: Vec<ChatMessage>,

    /// Active tool calls being streamed (for tracking partial tool calls)
    active_tool_calls: HashMap<String, PartialToolCall>,

    /// Recent tool calls for loop detection
    /// Contains (tool_name, arguments, timestamp)
    recent_tool_calls: VecDeque<(String, serde_json::Value, chrono::DateTime<chrono::Utc>)>,

    /// Optional system prompt
    system_prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PartialToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl<T: LlmProvider> Agent<T> {
    /// Create a new agent with the given LLM provider and tool registry
    pub fn new(
        llm_provider: T,
        tool_registry: ToolRegistry,
        system_prompt: Option<String>,
    ) -> Self {
        Self {
            llm_provider,
            tool_registry,
            conversation_history: Vec::new(),
            active_tool_calls: HashMap::new(),
            recent_tool_calls: VecDeque::new(),
            system_prompt,
        }
    }

    /// Get the conversation history
    #[allow(dead_code)]
    pub fn conversation_history(&self) -> &[ChatMessage] {
        &self.conversation_history
    }

    /// Add a message to the conversation history
    pub fn add_message(&mut self, message: ChatMessage) {
        self.conversation_history.push(message);
    }

    /// Clear the conversation history
    pub fn clear_history(&mut self) {
        self.conversation_history.clear();
        self.recent_tool_calls.clear();
    }

    /// Get a mutable reference to active tool calls (for streaming)
    pub fn active_tool_calls_mut(&mut self) -> &mut HashMap<String, PartialToolCall> {
        &mut self.active_tool_calls
    }

    /// Record a tool call for loop detection
    pub fn record_tool_call(
        &mut self,
        name: String,
        arguments: serde_json::Value,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) {
        self.recent_tool_calls
            .push_back((name, arguments, timestamp));

        // Keep only recent entries (limit to 20)
        while self.recent_tool_calls.len() > 20 {
            self.recent_tool_calls.pop_front();
        }
    }

    /// Check if a tool call would create a loop
    pub fn check_tool_loop(
        &mut self,
        tool_name: &str,
        arguments: &serde_json::Value,
        window_minutes: i64,
    ) -> usize {
        let now = chrono::Utc::now();
        let window = chrono::Duration::minutes(window_minutes);

        // Clean old entries
        self.recent_tool_calls
            .retain(|(_, _, timestamp)| now.signed_duration_since(*timestamp) < window);

        // Count duplicates
        self.recent_tool_calls
            .iter()
            .filter(|(name, args, _)| name == tool_name && args == arguments)
            .count()
    }

    /// Convert conversation history to LLM messages
    pub fn build_llm_messages(&self) -> Vec<LlmChatMessage> {
        let mut llm_messages = Vec::new();

        // Add system prompt if no system message exists
        let has_system_message = self
            .conversation_history
            .iter()
            .any(|msg| matches!(msg, ChatMessage::System { .. }));

        if !has_system_message {
            let prompt = if let Some(system_prompt) = &self.system_prompt {
                format!("You are an AI assistant. Here are your instructions:\n\n{system_prompt}")
            } else {
                "You are an AI assistant.".to_string()
            };
            llm_messages.push(LlmChatMessage::System { content: prompt });
        }

        // Convert conversation history
        let mut i = 0;
        while i < self.conversation_history.len() {
            let message = &self.conversation_history[i];

            match message {
                ChatMessage::System { content, .. } => {
                    llm_messages.push(LlmChatMessage::System {
                        content: content.clone(),
                    });
                }
                ChatMessage::User { content, .. } => {
                    llm_messages.push(LlmChatMessage::User {
                        content: content.clone(),
                    });
                }
                ChatMessage::Assistant { content, .. }
                | ChatMessage::AssistantStreaming { content, .. } => {
                    // Look ahead for tool calls
                    let mut tool_calls = Vec::new();
                    let mut j = i + 1;

                    while j < self.conversation_history.len() {
                        if let ChatMessage::ToolCall {
                            id, name, params, ..
                        } = &self.conversation_history[j]
                        {
                            if let Ok(arguments) = serde_json::from_str::<serde_json::Value>(params)
                            {
                                tool_calls.push(crate::llm::provider::ToolCall {
                                    id: id.clone(),
                                    name: name.clone(),
                                    arguments,
                                });
                            }
                            j += 1;
                        } else {
                            break;
                        }
                    }

                    llm_messages.push(LlmChatMessage::Assistant {
                        content: content.clone(),
                        tool_calls: if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        },
                    });

                    i = j - 1;
                }
                ChatMessage::ToolResult {
                    tool_call_id,
                    content,
                    ..
                } => {
                    llm_messages.push(LlmChatMessage::Tool {
                        tool_call_id: tool_call_id.clone(),
                        content: content.clone(),
                    });
                }
                ChatMessage::Tool { .. }
                | ChatMessage::ToolCall { .. }
                | ChatMessage::Error { .. } => {
                    // Skip these in LLM context
                }
            }
            i += 1;
        }

        llm_messages
    }

    /// Build tool definitions from the tool registry
    pub fn build_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_registry
            .list_tools()
            .into_iter()
            .filter_map(|tool_name| {
                let description = self.tool_registry.get_tool_description(&tool_name)?;
                let parameters = self.tool_registry.get_tool_parameters(&tool_name)?.clone();

                Some(ToolDefinition {
                    name: tool_name,
                    description,
                    parameters,
                })
            })
            .collect()
    }

    /// Create a chat request for the LLM
    pub fn create_chat_request(&self, temperature: Option<f32>) -> ChatRequest {
        ChatRequest {
            messages: self.build_llm_messages(),
            tools: self.build_tool_definitions(),
            temperature,
        }
    }

    /// Send a streaming request to the LLM
    pub async fn stream_completion(
        &self,
        temperature: Option<f32>,
    ) -> Result<crate::llm::provider::StreamChunkStream> {
        let request = self.create_chat_request(temperature);
        self.llm_provider
            .complete_stream_chunks(request)
            .await
            .map_err(|e| color_eyre::Report::msg(e.to_string()))
    }

    /// Update the last streaming message to a regular assistant message
    pub fn finalize_streaming_message(&mut self) {
        if let Some(last_message) = self.conversation_history.last().cloned() {
            if let ChatMessage::AssistantStreaming { content, timestamp } = last_message {
                if let Some(last_msg) = self.conversation_history.last_mut() {
                    *last_msg = ChatMessage::Assistant { content, timestamp };
                }
            }
        }
    }

    /// Append content to the current streaming message or create a new one
    pub fn append_streaming_content(&mut self, content: &str) {
        if let Some(ChatMessage::AssistantStreaming {
            content: current_content,
            timestamp: _,
        }) = self.conversation_history.last_mut()
        {
            // Append to existing streaming message
            current_content.push_str(content);
        } else {
            // Create new streaming message
            self.conversation_history
                .push(ChatMessage::AssistantStreaming {
                    content: content.to_string(),
                    timestamp: chrono::Utc::now(),
                });
        }
    }

    /// Get the tool registry
    #[allow(dead_code)]
    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    /// Get the LLM provider
    #[allow(dead_code)]
    pub fn llm_provider(&self) -> &T {
        &self.llm_provider
    }

    /// Get the server name for a given tool
    #[allow(dead_code)]
    pub fn get_server_for_tool(&self, tool_name: &str) -> Option<&String> {
        self.tool_registry.get_server_for_tool(tool_name)
    }

    /// Update the tool registry
    #[allow(dead_code)]
    pub fn update_tool_registry(&mut self, new_registry: ToolRegistry) {
        self.tool_registry = new_registry;
    }

    /// Execute a tool call using the tool registry
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.tool_registry.invoke_tool(tool_name, arguments).await
    }
}

use crate::{
    llm::provider::ChatMessage as LlmChatMessage,
    llm::{ChatRequest, LlmProvider, StreamEventStream},
    mcp::McpClient,
    tools::{Summarizer, ToolRegistry, TruncateSummarizer},
    types::{ChatMessage, IsoString, ToolDefinition},
};
use color_eyre::Result;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::SystemTime,
};

/// Represents an AI agent with its associated LLM provider, conversation context, and tools
pub struct Agent<T: LlmProvider> {
    /// The LLM provider (e.g., OpenRouter, Ollama) for this agent
    llm_provider: T,

    /// Tool registry containing available tools metadata
    tool_registry: ToolRegistry,

    /// MCP client for tool execution
    mcp_client: Option<Arc<McpClient>>,

    /// Summarizer for tool results
    summarizer: TruncateSummarizer,

    /// Conversation history for this agent
    conversation_history: Vec<ChatMessage>,

    /// Active tool calls being streamed (for tracking partial tool calls)
    active_tool_calls: HashMap<String, PartialToolCall>,

    /// Recent tool calls for loop detection
    /// Contains (tool_name, arguments, timestamp)
    recent_tool_calls: VecDeque<(String, serde_json::Value, SystemTime)>,

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
            mcp_client: None,
            summarizer: TruncateSummarizer::default(),
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
        timestamp: SystemTime,
    ) {
        self.recent_tool_calls
            .push_back((name, arguments, timestamp));

        // Keep only recent entries (limit to 20)
        while self.recent_tool_calls.len() > 20 {
            self.recent_tool_calls.pop_front();
        }
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
                    name: tool_name.clone(),
                    description,
                    parameters: parameters.to_string(),
                    server: self.tool_registry.get_server_for_tool(&tool_name).cloned(),
                })
            })
            .collect()
    }

    pub fn create_chat_request(&self, temperature: Option<f32>) -> ChatRequest {
        ChatRequest {
            messages: self.build_llm_messages(),
            tools: self.build_tool_definitions(),
            temperature,
        }
    }

    /// Send a streaming request to the LLM
    pub async fn stream_completion(&self, temperature: Option<f32>) -> Result<StreamEventStream> {
        let request = self.create_chat_request(temperature);
        self.llm_provider
            .complete_stream_chunks(request)
            .await
            .map_err(|e| color_eyre::Report::msg(e.to_string()))
    }

    /// Update the last streaming message to a regular assistant message
    pub fn finalize_streaming_message(&mut self) {
        if let Some(ChatMessage::AssistantStreaming { content, timestamp }) =
            self.conversation_history.last().cloned()
        {
            if let Some(last_msg) = self.conversation_history.last_mut() {
                *last_msg = ChatMessage::Assistant { content, timestamp };
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
                    timestamp: IsoString::now(),
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

    /// Set the MCP client
    pub fn set_mcp_client(&mut self, client: Arc<McpClient>) {
        self.mcp_client = Some(client);
    }

    /// Register tools from MCP client
    pub async fn register_mcp_tools(&mut self) -> Result<()> {
        if let Some(mcp_client) = &self.mcp_client {
            let tools = mcp_client.discover_tools().await?;
            for (server_name, tool) in tools {
                self.tool_registry.register_tool(server_name, tool);
            }
        }
        Ok(())
    }

    /// Execute a tool call using the MCP client
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Check if the tool exists in our registry
        if !self
            .tool_registry
            .list_tools()
            .contains(&tool_name.to_string())
        {
            return Err(color_eyre::Report::msg(format!(
                "Tool not found in registry: {tool_name}"
            )));
        }

        // Get the server name for this tool
        let server_name = self
            .tool_registry
            .get_server_for_tool(tool_name)
            .ok_or_else(|| {
                color_eyre::Report::msg(format!("Server not found for tool: {tool_name}"))
            })?;

        // Get the MCP client
        let mcp_client = self
            .mcp_client
            .as_ref()
            .ok_or_else(|| color_eyre::Report::msg("No MCP client available"))?;

        // Execute the tool
        let result = mcp_client
            .execute_tool(server_name, tool_name, arguments)
            .await?;

        // Apply summarization/truncation to the result
        let result_str = serde_json::to_string(&result)?;
        let processed_result = self.summarizer.summarize(&result_str).await?;

        Ok(serde_json::Value::String(processed_result))
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
                                tool_calls.push(crate::types::ToolCall {
                                    id: id.clone(),
                                    name: name.clone(),
                                    arguments: arguments.to_string(),
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
}

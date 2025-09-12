use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::McpManager;
use crate::mcp::builtin_servers::CodingMcp;
use crate::types::{ChatMessage, IsoString, LlmResponse};
use async_stream::stream;
use color_eyre::Result;
use futures::StreamExt;
use futures::pin_mut;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tokio_stream::Stream;

pub enum McpServerConfig {
    Http {
        name: String,
        config: StreamableHttpClientTransportConfig,
    },

    Stdio {
        name: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BuiltinMcp {
    Coding,
}

pub fn agent<T: ModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder {
        llm,
        system_prompt: None,
        mcp_manager: McpManager::new(),
        mcp_configs: Vec::new(),
        builtin_configs: HashSet::new(),
    }
}

pub struct AgentBuilder<T: ModelProvider> {
    llm: T,
    system_prompt: Option<String>,
    mcp_manager: McpManager,
    mcp_configs: Vec<McpServerConfig>,
    builtin_configs: HashSet<BuiltinMcp>,
}

impl<T: ModelProvider + 'static> AgentBuilder<T> {
    pub fn system(mut self, prompt: &str) -> Self {
        self.system_prompt = Some(prompt.to_string());
        self
    }

    pub fn mcp(mut self, config: McpServerConfig) -> Self {
        self.mcp_configs.push(config);
        self
    }

    pub fn coding_tools(mut self) -> Self {
        self.builtin_configs.insert(BuiltinMcp::Coding);
        self
    }

    pub async fn build(self) -> Result<Agent<T>> {
        let mut messages = Vec::new();

        if let Some(system_prompt) = &self.system_prompt {
            messages.push(ChatMessage::System {
                content: system_prompt.clone(),
                timestamp: IsoString::now(),
            });
        }

        let mut mcp_manager = self.mcp_manager;

        for builtin in self.builtin_configs {
            match builtin {
                BuiltinMcp::Coding => {
                    mcp_manager
                        .with_in_memory_mcp("coding".to_string(), CodingMcp::new())
                        .await?;
                }
            }
        }

        for config in self.mcp_configs {
            match config {
                McpServerConfig::Http { name, config } => {
                    mcp_manager.with_http_mcp(&name, &config).await?;
                }

                McpServerConfig::Stdio {
                    name,
                    command,
                    args,
                    env,
                } => {
                    mcp_manager.with_stdio_mcp(name, command, args, env).await?;
                }
            }
        }

        Ok(Agent {
            llm: self.llm,
            mcp_client: mcp_manager,
            messages,
        })
    }

    pub async fn spawn(self) -> Result<(mpsc::Sender<UserMessage>, mpsc::Receiver<AgentMessage>)> {
        let (user_tx, mut user_rx) = mpsc::channel::<UserMessage>(100);
        let (agent_tx, agent_rx) = mpsc::channel::<AgentMessage>(100);

        let mut agent = self.build().await?;

        tokio::spawn(async move {
            while let Some(message) = user_rx.recv().await {
                let result_stream = agent.send(message).await;
                pin_mut!(result_stream);

                while let Some(event) = result_stream.next().await {
                    if agent_tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
        });

        Ok((user_tx, agent_rx))
    }
}

pub struct Agent<T: ModelProvider> {
    llm: T,
    mcp_client: McpManager,
    messages: Vec<ChatMessage>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub async fn send(&mut self, message: UserMessage) -> impl Stream<Item = AgentMessage> + Send {
        match message {
            UserMessage::Text { content } => {
                let user_message = ChatMessage::User {
                    content,
                    timestamp: IsoString::now(),
                };

                self.messages.push(user_message);
                self.run_agent_loop().await
            }
        }
    }

    async fn run_agent_loop(&mut self) -> impl Stream<Item = AgentMessage> + Send {
        const MAX_ITERATIONS: usize = 10;
        let mut n_iterations = 0;

        stream! {
            match self.mcp_client.discover_tools().await  {
                Ok(_) => {}
                Err(e) => {
                    yield AgentMessage::Error {
                        message: format!("Failed to discover tools: {}", e),
                    };
                    return
                }
            };

            loop {
                if n_iterations >= MAX_ITERATIONS {
                    yield AgentMessage::Error {
                        message: "Maximum recursion depth reached".to_string(),
                    };
                    break;
                }

                let tools = self.mcp_client.get_tool_definitions();
                let messages_clone = self.messages.clone();

                let mut current_message_id = None;
                let mut accumulated_content = String::new();
                let mut completed_tool_calls: Vec<(crate::types::ToolCallRequest, String)> = Vec::new();
                let mut has_tool_calls = false;

                let llm_stream = self.llm.generate_response(Context {
                    messages: messages_clone,
                    tools,
                });

                pin_mut!(llm_stream);

                while let Some(event) = llm_stream.next().await {
                    match event {
                        Ok(LlmResponse::Start { message_id }) => {
                            current_message_id = Some(message_id);
                        }
                        Ok(LlmResponse::Text { chunk }) => {
                            accumulated_content.push_str(&chunk);

                            if let Some(message_id) = &current_message_id {
                                yield AgentMessage::Text {
                                    message_id: message_id.clone(),
                                    chunk,
                                    is_complete: false,
                                };
                            }
                        }
                        Ok(LlmResponse::ToolRequestStart { id, name }) => {
                            yield AgentMessage::ToolCall {
                                tool_call_id: id,
                                name,
                                arguments: None,
                                result: None,
                                is_complete: false,
                            };
                        }
                        Ok(LlmResponse::ToolRequestArg { id, chunk }) => {
                            yield AgentMessage::ToolCall {
                                tool_call_id: id,
                                name: String::new(), // Name will be available from the start event
                                arguments: Some(chunk),
                                result: None,
                                is_complete: false,
                            };
                        }
                        Ok(LlmResponse::ToolRequestComplete { tool_call }) => {
                            let result_str = match serde_json::from_str(&tool_call.arguments) {
                                Ok(args) => {
                                    match self.mcp_client.execute_tool(&tool_call.name, args).await {
                                        Ok(result) => result.to_string(),
                                        Err(e) => format!("Tool execution failed: {}", e),
                                    }
                                }
                                Err(e) => format!("Invalid tool arguments: {}", e),
                            };

                            // Store tool result but don't add to messages yet - wait for LLM Done
                            yield AgentMessage::ToolCall {
                                tool_call_id: tool_call.id.clone(),
                                name: tool_call.name.clone(),
                                arguments: None,
                                result: Some(result_str.clone()),
                                is_complete: true,
                            };

                            // Store the tool call and result for later
                            completed_tool_calls.push((tool_call, result_str));
                            has_tool_calls = true;
                        }
                        Ok(LlmResponse::Done) => {
                            // Send final message chunk to indicate completion
                            if let Some(message_id) = &current_message_id {
                                yield AgentMessage::Text {
                                    message_id: message_id.clone(),
                                    chunk: String::new(),
                                    is_complete: true,
                                };
                            }

                            // Add the completed assistant message to conversation history first
                            let tool_call_requests: Vec<_> = completed_tool_calls
                                .iter()
                                .map(|(tool_call, _)| tool_call.clone())
                                .collect();

                            self.messages.push(ChatMessage::Assistant {
                                content: accumulated_content,
                                timestamp: IsoString::now(),
                                tool_calls: tool_call_requests,
                            });

                            // Then add tool results in the correct order
                            for (tool_call, result_str) in completed_tool_calls {
                                self.messages.push(ChatMessage::ToolCallResult {
                                    tool_call_id: tool_call.id,
                                    content: result_str,
                                    timestamp: IsoString::now(),
                                });
                            }

                            if has_tool_calls {
                                n_iterations += 1;
                                break;
                            } else {
                                return;
                            }
                        }
                        Ok(LlmResponse::Error { message }) => {
                            yield AgentMessage::Error { message };
                            return;
                        }
                        Err(e) => {
                            yield AgentMessage::Error {
                                message: e.to_string(),
                            };
                            return;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AgentMessage {
    Text {
        message_id: String,
        chunk: String,
        is_complete: bool,
    },

    ToolCall {
        tool_call_id: String,
        name: String,
        arguments: Option<String>,
        result: Option<String>,
        is_complete: bool,
    },

    Error {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum UserMessage {
    Text { content: String },
}

impl UserMessage {
    pub fn text(content: &str) -> Self {
        UserMessage::Text {
            content: content.to_string(),
        }
    }
}

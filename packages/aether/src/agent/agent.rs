use crate::agent::AgentMessage;
use crate::agent::UserMessage;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager};
use crate::types::ToolCallRequest;
use crate::types::{ChatMessage, IsoString, LlmResponse};
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct Agent<T: ModelProvider> {
    llm: Arc<Mutex<T>>,
    mcp_client: Arc<Mutex<McpManager>>,
    context: Arc<Mutex<Context>>,
    cancellation_token: CancellationToken,
    elicitation_receiver: mpsc::UnboundedReceiver<ElicitationRequest>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(
        llm: T,
        mcp_client: McpManager,
        messages: Vec<ChatMessage>,
        elicitation_receiver: mpsc::UnboundedReceiver<ElicitationRequest>,
    ) -> Self {
        Self {
            llm: Arc::new(Mutex::new(llm)),
            mcp_client: Arc::new(Mutex::new(mcp_client)),
            context: Arc::new(Mutex::new(Context {
                messages,
                tools: Vec::new(), // Will be populated when tools are discovered
            })),
            cancellation_token: CancellationToken::new(),
            elicitation_receiver,
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.lock().await.display_name()
    }

    pub async fn send(
        &mut self,
        message: UserMessage,
    ) -> mpsc::Receiver<AgentMessage> {
        let (tx, rx) = mpsc::channel(100);
        self.run_agent_loop(message, tx).await;
        rx
    }

    async fn run_agent_loop(
        &mut self,
        message: UserMessage,
        tx: mpsc::Sender<AgentMessage>,
    ) {
        // Handle the incoming message and set up cancellation
        match message {
            UserMessage::Text { content } => {
                let user_message = ChatMessage::User {
                    content,
                    timestamp: IsoString::now(),
                };
                self.context.lock().await.messages.push(user_message);
                self.cancellation_token = CancellationToken::new();
            }
            UserMessage::Cancel => {
                self.cancellation_token.cancel();
                let _ = tx.send(AgentMessage::Cancelled {
                    message: "Operation was cancelled".to_string(),
                }).await;
                return;
            }
        };

        // Clone shared state for the spawned task
        let cancellation_token = self.cancellation_token.clone();
        let context = self.context.clone();
        let mcp_client = self.mcp_client.clone();
        let llm = self.llm.clone();
        let mut elicitation_receiver = std::mem::replace(
            &mut self.elicitation_receiver,
            mpsc::unbounded_channel().1, // Create a dummy receiver
        );

        tokio::spawn(async move {
            const MAX_ITERATIONS: usize = 10_000;
            let mut n_iterations = 0;

            if cancellation_token.is_cancelled() {
                let _ = tx.send(AgentMessage::Cancelled {
                    message: "Operation was cancelled".to_string(),
                }).await;
                return;
            }

            // Discover tools
            {
                let mut mcp_client_guard = mcp_client.lock().await;
                let mut context_guard = context.lock().await;
                match mcp_client_guard.discover_tools().await {
                    Ok(_) => { context_guard.tools = mcp_client_guard.get_tool_definitions(); }
                    Err(e) => {
                        let _ = tx.send(AgentMessage::Error {
                            message: format!("Failed to discover tools: {}", e),
                        }).await;
                        return;
                    }
                }
            }

            // Main agent loop
            loop {
                if cancellation_token.is_cancelled() {
                    let _ = tx.send(AgentMessage::Cancelled {
                        message: "Operation was cancelled during agent loop".to_string(),
                    }).await;
                    return;
                }

                if n_iterations >= MAX_ITERATIONS {
                    let _ = tx.send(AgentMessage::Error {
                        message: "Maximum recursion depth reached".to_string(),
                    }).await;
                    break;
                }

                // Check for elicitation requests
                while let Ok(elicitation_request) = elicitation_receiver.try_recv() {
                    let request_id = Uuid::new_v4().to_string();
                    let _ = tx.send(AgentMessage::ElicitationRequest {
                        request_id,
                        request: elicitation_request.request,
                        response_sender: elicitation_request.response_sender,
                    }).await;
                }

                let mut current_message_id = None;
                let mut accumulated_content = String::new();
                let mut has_tool_calls = false;
                let mut tool_call_requests: Vec<ToolCallRequest> = Vec::new();

                let (context_guard, llm_guard) = (context.lock().await, llm.lock().await);
                let llm_stream = llm_guard.stream_response(&context_guard);
                let model_name = llm_guard.display_name();
                drop((context_guard, llm_guard)); // Release the locks
                pin_mut!(llm_stream);

                // Main event loop
                loop {
                    tokio::select! {
                        // Handle LLM stream events
                        llm_event = llm_stream.next() => {
                            match llm_event {
                                Some(event) => {
                                    if cancellation_token.is_cancelled() {
                                        let _ = tx.send(AgentMessage::Cancelled {
                                            message: "Operation was cancelled".to_string(),
                                        }).await;
                                        return;
                                    }

                                    use LlmResponse::*;
                                    match event {
                                        Ok(Start { message_id }) => {
                                            current_message_id = Some(message_id);
                                        }
                                        Ok(Text { chunk }) => {
                                            accumulated_content.push_str(&chunk);

                                            if let Some(message_id) = &current_message_id {
                                                let _ = tx.send(AgentMessage::Text {
                                                    message_id: message_id.clone(),
                                                    chunk,
                                                    is_complete: false,
                                                    model_name: model_name.clone(),
                                                }).await;
                                            }
                                        }
                                        Ok(ToolRequestStart { id, name }) => {
                                            let _ = tx.send(AgentMessage::ToolCall {
                                                tool_call_id: id,
                                                name,
                                                arguments: None,
                                                result: None,
                                                is_complete: false,
                                                model_name: model_name.clone(),
                                            }).await;
                                        }
                                        Ok(ToolRequestArg { id, chunk }) => {
                                            let _ = tx.send(AgentMessage::ToolCall {
                                                tool_call_id: id,
                                                name: String::new(),
                                                arguments: Some(chunk),
                                                result: None,
                                                is_complete: false,
                                                model_name: model_name.clone(),
                                            }).await;
                                        }
                                        Ok(ToolRequestComplete { tool_call }) => {
                                            // Store the tool call request for context
                                            tool_call_requests.push(tool_call.clone());
                                            has_tool_calls = true;

                                            // Spawn tool execution as a separate task
                                            let tx_clone = tx.clone();
                                            let tool_call_clone = tool_call.clone();
                                            let context_clone = context.clone();
                                            let model_name_clone = model_name.clone();

                                            tokio::spawn({
                                                let mcp_client = mcp_client.clone();
                                                async move {
                                                    let result_str = match serde_json::from_str(&tool_call_clone.arguments) {
                                                        Ok(args) => {
                                                            let mcp_client_guard = mcp_client.lock().await;
                                                            match mcp_client_guard.execute_tool(&tool_call_clone.name, args).await {
                                                                Ok(result) => result.to_string(),
                                                                Err(e) => format!("Tool execution failed: {}", e),
                                                            }
                                                        }
                                                        Err(e) => format!("Invalid tool arguments: {}", e),
                                                    };

                                                    // Send result directly to the output channel
                                                    let _ = tx_clone.send(AgentMessage::ToolCall {
                                                        tool_call_id: tool_call_clone.id.clone(),
                                                        name: tool_call_clone.name.clone(),
                                                        arguments: None,
                                                        result: Some(result_str.clone()),
                                                        is_complete: true,
                                                        model_name: model_name_clone,
                                                    }).await;

                                                    // Update context with tool result
                                                    let mut context_guard = context_clone.lock().await;
                                                    context_guard.messages.push(ChatMessage::ToolCallResult {
                                                        tool_call_id: tool_call_clone.id,
                                                        content: result_str,
                                                        timestamp: IsoString::now(),
                                                    });
                                                }
                                            });
                                        }
                                        Ok(Done) => {
                                            if let Some(message_id) = &current_message_id {
                                                let _ = tx.send(AgentMessage::Text {
                                                    message_id: message_id.clone(),
                                                    chunk: String::new(),
                                                    is_complete: true,
                                                    model_name: model_name.clone(),
                                                }).await;
                                            }

                                            // Add assistant message with tool calls to context
                                            {
                                                let mut context_guard = context.lock().await;
                                                context_guard.messages.push(ChatMessage::Assistant {
                                                    content: accumulated_content,
                                                    timestamp: IsoString::now(),
                                                    tool_calls: tool_call_requests,
                                                });
                                            }

                                            if has_tool_calls {
                                                n_iterations += 1;
                                                break; // Break inner loop to continue outer loop
                                            } else {
                                                return; // No tool calls, we're done
                                            }
                                        }
                                        Ok(Error { message }) => {
                                            let _ = tx.send(AgentMessage::Error { message }).await;
                                            return;
                                        }
                                        Err(e) => {
                                            let _ = tx.send(AgentMessage::Error {
                                                message: e.to_string(),
                                            }).await;
                                            return;
                                        }
                                    }
                                }
                                None => {
                                    // Stream ended unexpectedly
                                    let _ = tx.send(AgentMessage::Error {
                                        message: "LLM stream ended unexpectedly".to_string(),
                                    }).await;
                                    return;
                                }
                            }
                        }

                        // Handle elicitation requests
                        elicitation_request = elicitation_receiver.recv() => {
                            if let Some(elicitation_request) = elicitation_request {
                                let request_id = Uuid::new_v4().to_string();
                                let _ = tx.send(AgentMessage::ElicitationRequest {
                                    request_id,
                                    request: elicitation_request.request,
                                    response_sender: elicitation_request.response_sender,
                                }).await;
                            }
                        }
                    }
                }
            }
        });
    }
}

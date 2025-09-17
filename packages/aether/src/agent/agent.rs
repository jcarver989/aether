use crate::agent::AgentMessage;
use crate::agent::UserMessage;
use crate::agent::tool_execution_task::ToolExecutionTask;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager};
use crate::types::ChatMessage;
use crate::types::IsoString;
use crate::types::LlmResponse;
use crate::types::ToolCallRequest;
use async_stream::stream;
use color_eyre::{Report, Result};
use futures::Stream;
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

pub struct Agent<T: ModelProvider> {
    llm: T,
    mcp_client: Arc<Mutex<McpManager>>,
    context: Context,
    current_task_token: Option<CancellationToken>,
    elicitation_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ElicitationRequest>>>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(
        llm: T,
        mcp_client: McpManager,
        messages: Vec<ChatMessage>,
        elicitation_receiver: mpsc::UnboundedReceiver<ElicitationRequest>,
    ) -> Self {
        Self {
            llm: llm,
            mcp_client: Arc::new(Mutex::new(mcp_client)),
            context: Context::new(
                messages,
                Vec::new(), // populated when tools are discovered
            ),
            current_task_token: None,
            elicitation_receiver: Arc::new(Mutex::new(elicitation_receiver)),
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    pub async fn send(
        &mut self,
        message: UserMessage,
    ) -> (impl Stream<Item = AgentMessage>, CancellationToken) {
        match message {
            UserMessage::Text { content } => {
                if let Some(token) = &self.current_task_token {
                    token.cancel();
                }

                let cancellation_token = CancellationToken::new();
                self.current_task_token = Some(cancellation_token.clone());
                self.context.add_message(ChatMessage::User {
                    content,
                    timestamp: IsoString::now(),
                });

                let stream = self.process_user_message().await;
                (stream, cancellation_token)
            }
        }
    }

    async fn update_tools(&mut self) -> Result<()> {
        let mut mcp = self.mcp_client.lock().await;
        mcp.discover_tools().await?;
        let tools = mcp.get_tool_definitions();
        self.context.set_tools(tools);
        Ok(())
    }

    async fn process_user_message(&mut self) -> impl Stream<Item = AgentMessage> {
        stream! {
            match self.update_tools().await {
                Ok(_) => {}
                Err(e) => {
                    yield AgentMessage::Error { message: format!("Error fetching tools: {:?}", e) };
                    return;
                }
            };

            let mut tool_collector = ToolResultsCollector::new();

            // Main "agentic" loop.
            // Each iteration of the outer loop procesess 1 LLM call
            // Each iteration of the inner loop processes 1 streaming "event" chunk from the LLM's response
            loop {
                let response_stream = self.llm.stream_response(&self.context);
                let model_name = self.llm.display_name();

                pin_mut!(response_stream);

                let mut current_message_id: Option<String> = None;
                let mut message_content = String::new();

                loop {
                    while let Some(tool_result) = tool_collector.try_recv_result() {
                        yield AgentMessage::ToolCall {
                            tool_call_id: tool_result.id.clone(),
                            name: tool_result.name.clone(),
                            arguments: Some(tool_result.arguments.clone()),
                            result: Some(tool_result.result),
                            is_complete: true,
                            model_name: model_name.clone(),
                        };
                    }

                    if let Some(event) = response_stream.next().await {
                        use LlmResponse::*;
                        match event {
                            Ok(Start { message_id}) => {
                                current_message_id = Some(message_id);
                            }

                            Ok(Text { chunk}) => {
                                message_content.push_str(&chunk);
                                if let Some(ref id) = current_message_id {
                                    yield AgentMessage::Text {
                                        message_id: id.clone(),
                                        chunk,
                                        is_complete: false,
                                        model_name: model_name.clone()
                                    };
                                }
                            }

                            Ok(ToolRequestStart { id, name}) => {
                                yield AgentMessage::ToolCall {
                                    tool_call_id: id,
                                    name,
                                    arguments: None,
                                    result: None,
                                    is_complete: false,
                                    model_name: model_name.clone()
                                };
                            }

                            Ok(ToolRequestArg { id, chunk}) => {
                                yield AgentMessage::ToolCall {
                                    tool_call_id: id,
                                    name: String::new(),
                                    arguments: Some(chunk.to_string()),
                                    result: None,
                                    is_complete: false,
                                    model_name: model_name.clone()
                                };
                            }

                            Ok(ToolRequestComplete { tool_call}) => {
                                let tool_tx = tool_collector.start_tool_request(tool_call.clone());
                                let task = ToolExecutionTask::new(
                                    self.mcp_client.clone(),
                                    tool_call.clone(),
                                    tool_tx,
                                );

                                tokio::spawn(task.run());
                            }

                            Ok(Done) => {
                                let tool_results = tool_collector.get_all_results();
                                self.context.add_message(ChatMessage::Assistant {
                                    content: message_content.clone(),
                                    timestamp: IsoString::now(),
                                    tool_calls: tool_collector.requests.clone()
                                });

                                for result in tool_results {
                                    self.context.add_message(ChatMessage::ToolCallResult {
                                        tool_call_id: result.id,
                                        content: result.result,
                                        timestamp: IsoString::now()
                                    });
                                }

                                if let Some(ref id) = current_message_id {
                                    yield AgentMessage::Text {
                                        message_id: id.clone(),
                                        chunk: String::new(),
                                        is_complete: true,
                                        model_name: model_name.clone()
                                    };
                                }

                                if tool_collector.requests.is_empty() {
                                    return;
                                }

                                tool_collector = ToolResultsCollector::new();
                                break;
                            }

                            Ok(Error { message }) => {
                                yield AgentMessage::Error { message: message.to_string() };
                                return;
                            }

                            Err(e) => {
                                yield AgentMessage::Error { message: e.to_string() };
                                return;
                            }
                        }
                    } else {
                        yield AgentMessage::Error { message: "Empty LLM stream".to_string() };
                        return;
                    }
                }
            }
        }
    }
}

struct ToolResultsCollector {
    requests: Vec<ToolCallRequest>,
    tool_result_rx: mpsc::UnboundedReceiver<ToolCallResult>,
    tool_result_tx: mpsc::UnboundedSender<ToolCallResult>,
    completed_results: Vec<ToolCallResult>,
}

impl ToolResultsCollector {
    pub fn new() -> Self {
        let (tool_result_tx, tool_result_rx) = mpsc::unbounded_channel();
        Self {
            requests: Vec::new(),
            tool_result_rx,
            tool_result_tx,
            completed_results: Vec::new(),
        }
    }

    pub fn start_tool_request(
        &mut self,
        request: ToolCallRequest,
    ) -> mpsc::UnboundedSender<ToolCallResult> {
        self.requests.push(request);
        self.tool_result_tx.clone()
    }

    pub fn try_recv_result(&mut self) -> Option<ToolCallResult> {
        if let Ok(result) = self.tool_result_rx.try_recv() {
            self.completed_results.push(result.clone());
            Some(result)
        } else {
            None
        }
    }

    pub fn get_all_results(&mut self) -> Vec<ToolCallResult> {
        // Collect any remaining results from channel first
        while let Ok(result) = self.tool_result_rx.try_recv() {
            self.completed_results.push(result);
        }

        self.completed_results.clone()
    }
}

#[derive(Clone)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
}

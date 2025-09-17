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
use futures::Stream;
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

pub struct Agent<T: ModelProvider> {
    llm: Arc<T>,
    mcp_client: Arc<Mutex<McpManager>>,
    context: Arc<StdMutex<Context>>,
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
            llm: Arc::new(llm),
            mcp_client: Arc::new(Mutex::new(mcp_client)),
            context: Arc::new(StdMutex::new(Context::new(
                messages,
                Vec::new(), // populated when tools are discovered
            ))),
            current_task_token: None,
            elicitation_receiver: Arc::new(Mutex::new(elicitation_receiver)),
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    pub fn send(
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
                {
                    let mut context = self.context.lock().unwrap();
                    context.add_message(ChatMessage::User {
                        content,
                        timestamp: IsoString::now(),
                    });
                };

                let stream = Self::run_agent_loop(
                    self.mcp_client.clone(),
                    self.context.clone(),
                    self.llm.clone(),
                );

                (stream, cancellation_token)
            }
        }
    }

    fn run_agent_loop(
        mcp_client: Arc<Mutex<McpManager>>,
        context: Arc<StdMutex<Context>>,
        llm: Arc<T>,
    ) -> impl Stream<Item = AgentMessage> {
        stream! {
            let mut tool_collector = ToolResultsCollector::new();

            loop {
                let response_stream = {
                    let context = context.lock().unwrap();
                    llm.stream_response(&context)
                };

                let model_name = llm.display_name();
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
                                    mcp_client.clone(),
                                    tool_call.clone(),
                                    tool_tx,
                                );

                                tokio::spawn(task.run());
                            }

                            Ok(Done) => {
                                // Collect all tool results for context management
                                let all_results = tool_collector.get_all_results();
                                {
                                    let mut context = context.lock().unwrap();
                                    context.add_message(ChatMessage::Assistant {
                                        content: message_content.clone(),
                                        timestamp: IsoString::now(),
                                        tool_calls: tool_collector.requests.clone()
                                    });

                                    for result in all_results {
                                        context.add_message(ChatMessage::ToolCallResult {
                                            tool_call_id: result.id,
                                            content: result.result,
                                            timestamp: IsoString::now()
                                        });
                                    }
                                };

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

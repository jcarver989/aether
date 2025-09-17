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
use futures::StreamExt;
use futures::future::join_all;
use futures::pin_mut;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::sync::{Mutex, mpsc};
use std::sync::{ Mutex as StdMutex }
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

    pub async fn send(
        &mut self,
        message: UserMessage,
    ) -> (mpsc::Receiver<AgentMessage>, CancellationToken) {
        let (tx, rx) = mpsc::channel(100);

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

                tokio::spawn(
                    Self::run_agent_loop(
                    self.mcp_client.clone(),
                    self.context.clone(), self.llm.clone(), tx));
                (rx, cancellation_token)
            }
        }
    }

    async fn run_agent_loop(
        mcp_client: Arc<Mutex<McpManager>>,
        context: Arc<StdMutex<Context>>, 
        llm: Arc<T>, 
        tx: Sender<AgentMessage>) -> () {
        loop {
            let response_stream =  { 
                let context = context.lock().unwrap();
                llm.stream_response(&context)
            };

            let model_name = llm.display_name();
            let mut tool_collector = ToolResultsCollector::new();
            pin_mut!(response_stream);

            loop {
                let mut current_message_id: Option<String> = None;
                let mut message_content = String::new();

                tokio::select! {
                    event = response_stream.next() => {
                        use LlmResponse::*;
                        match event {
                            Some(Ok(Start { message_id})) => {
                                current_message_id = Some(message_id);
                            }

                            Some(Ok(Text { chunk})) => {
                                message_content.push_str(&chunk);
                                if let Some(id) = current_message_id {
                                    tx.send(AgentMessage::Text { message_id: id.clone(), chunk, is_complete: false, model_name: model_name.clone()  }).await;
                                }
                            }

                            Some(Ok(ToolRequestStart { id, name})) => {
                                tx.send(AgentMessage::ToolCall { tool_call_id: id, name, arguments: None, result: None, is_complete: false, model_name: model_name.clone() }).await;
                            }

                            Some(Ok(ToolRequestArg { id, chunk})) => {
                                tx.send(AgentMessage::ToolCall { tool_call_id: id, name: String::new(), arguments: Some(chunk.to_string()), result: None, is_complete: false, model_name: model_name.clone() }).await;
                            }

                            Some(Ok(ToolRequestComplete { tool_call})) => {
                               let tool_tx = tool_collector.start_tool_request(tool_call.clone());
                               let task = ToolExecutionTask::new(
                                    mcp_client.clone(),
                                    tx.clone(),
                                    tool_call.clone(),
                                    model_name.clone(),
                                    tool_tx,
                                );

                                tokio::spawn(task.run());
                            }

                            Some(Ok(Done)) => {
                                let (requests, results) = tool_collector.collect_results().await;
                                {
                                    let mut context = context.lock().unwrap();
                                    context.add_message(ChatMessage::Assistant { content: message_content, timestamp: IsoString::now(), tool_calls: requests });

                                    for result in results {
                                        context.add_message(ChatMessage::ToolCallResult { tool_call_id: result.id, content: result.result, timestamp: IsoString::now() });
                                    }
                                };

                                if let Some(id) = current_message_id {
                                    tx.send(AgentMessage::Text {
                                        message_id: id,
                                        chunk: String::new(),
                                        is_complete: true,
                                        model_name: model_name.clone()
                                    }).await;
                                }
                            }


                            Some(Ok(Error { message })) => {
                                tx.send(AgentMessage::Error { message: message.to_string() }).await;
                                return;
                            }

                            Some(Err(e)) => {
                                tx.send(AgentMessage::Error { message: e.to_string() }).await;
                                return;
                            }

                            None => {
                                tx.send(AgentMessage::Error { message: "Empty LLM stream".to_string() }).await;
                                return
                            }
                        }
                    }
                }
            }
        }
    }
}

struct ToolResultsCollector {
    requests: Vec<ToolCallRequest>,
    results: Vec<oneshot::Receiver<ToolCallResult>>,
}

impl ToolResultsCollector {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            results: Vec::new(),
        }
    }

    pub fn start_tool_request(
        &mut self,
        request: ToolCallRequest,
    ) -> oneshot::Sender<ToolCallResult> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.requests.push(request);
        self.results.push(result_receiver);
        result_sender
    }

    pub async fn collect_results(&mut self) -> (Vec<ToolCallRequest>, Vec<ToolCallResult>) {
        let mut tool_results = Vec::<ToolCallResult>::new();
        if !self.results.is_empty() {
            let receivers = std::mem::take(&mut self.results);
            let results = join_all(receivers).await;
            for result in results {
                if let Ok(result) = result {
                    tool_results.push(result);
                }
            }
        }

        (self.requests.clone(), tool_results)
    }
}

pub struct ToolCallResult {
    pub id: String,
    pub result: String,
}

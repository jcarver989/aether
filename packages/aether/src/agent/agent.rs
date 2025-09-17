use crate::agent::AgentMessage;
use crate::agent::UserMessage;
use crate::agent::agent_task::AgentTask;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager};
use crate::types::{ChatMessage, IsoString};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

pub struct Agent<T: ModelProvider> {
    llm: Arc<Mutex<T>>,
    mcp_client: Arc<Mutex<McpManager>>,
    context: Arc<Mutex<Context>>,
    cancellation_token: CancellationToken,
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
            llm: Arc::new(Mutex::new(llm)),
            mcp_client: Arc::new(Mutex::new(mcp_client)),
            context: Arc::new(Mutex::new(Context {
                messages,
                tools: Vec::new(), // Will be populated when tools are discovered
            })),
            cancellation_token: CancellationToken::new(),
            elicitation_receiver: Arc::new(Mutex::new(elicitation_receiver)),
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.lock().await.display_name()
    }

    pub async fn send(&mut self, message: UserMessage) -> mpsc::Receiver<AgentMessage> {
        let (tx, rx) = mpsc::channel(100);

        match message {
            UserMessage::Text { content } => {
                self.cancellation_token = CancellationToken::new();
                self.context.lock().await.messages.push(ChatMessage::User {
                    content,
                    timestamp: IsoString::now(),
                });

                let task = AgentTask::new(
                    self.cancellation_token.clone(),
                    self.context.clone(),
                    self.mcp_client.clone(),
                    self.llm.clone(),
                    self.elicitation_receiver.clone(),
                    tx,
                );

                tokio::spawn(task.run());
            }
            UserMessage::Cancel => {
                self.cancellation_token.cancel();
                let _ = tx
                    .send(AgentMessage::Cancelled {
                        message: "Operation was cancelled".to_string(),
                    })
                    .await;
            }
        };

        rx
    }

}

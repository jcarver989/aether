use crate::agent::AgentMessage;
use crate::agent::UserMessage;
use crate::agent::process_user_message_task::ProcessUserMessageTask;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager};
use crate::types::ChatMessage;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

pub struct Agent<T: ModelProvider> {
    llm: Arc<Mutex<T>>,
    mcp_client: Arc<Mutex<McpManager>>,
    context: Arc<Mutex<Context>>,
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
            llm: Arc::new(Mutex::new(llm)),
            mcp_client: Arc::new(Mutex::new(mcp_client)),
            context: Arc::new(Mutex::new(Context::new(
                messages,
                Vec::new(), // Will be populated when tools are discovered
            ))),
            current_task_token: None,
            elicitation_receiver: Arc::new(Mutex::new(elicitation_receiver)),
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.lock().await.display_name()
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

                let message_token = CancellationToken::new();
                self.current_task_token = Some(message_token.clone());

                self.context.lock().await.add_user_message(content);

                let task = ProcessUserMessageTask::new(
                    message_token.clone(),
                    self.context.clone(),
                    self.mcp_client.clone(),
                    self.llm.clone(),
                    self.elicitation_receiver.clone(),
                    tx,
                );

                tokio::spawn(task.run());
                (rx, message_token)
            }
            UserMessage::Cancel => {
                let cancel_token = CancellationToken::new();
                cancel_token.cancel(); // Pre-cancelled token

                if let Some(token) = &self.current_task_token {
                    token.cancel();
                }
                self.current_task_token = None;

                let _ = tx
                    .send(AgentMessage::Cancelled {
                        message: "Operation was cancelled".to_string(),
                    })
                    .await;

                (rx, cancel_token)
            }
        }
    }
}

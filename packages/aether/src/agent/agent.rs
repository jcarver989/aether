use crate::agent::AgentMessage;
use crate::agent::UserMessage;
use crate::agent::agent_task::AgentTask;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::McpManager;
use crate::types::ChatMessage;
use crate::types::IsoString;
use crate::types::ToolCallRequest;
use futures::Stream;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::Mutex as TokioMutex;

pub struct Agent<T: ModelProvider> {
    llm: Arc<T>,
    mcp: Arc<TokioMutex<McpManager>>,
    context: Arc<Mutex<Context>>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(llm: T, mcp_manager: McpManager, messages: Vec<ChatMessage>) -> Self {
        let mcp = Arc::new(TokioMutex::new(mcp_manager));

        Self {
            llm: Arc::new(llm),
            mcp,
            context: Arc::new(Mutex::new(Context::new(
                messages,
                Vec::new(), // populated when tools are discovered
            ))),
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    pub async fn send(&mut self, message: UserMessage) -> impl Stream<Item = AgentMessage> {
        match message {
            UserMessage::Text { content } => {
                self.context.lock().unwrap().add_message(ChatMessage::User {
                    content,
                    timestamp: IsoString::now(),
                });

                let (_handle, rx) =
                    AgentTask::run(self.llm.clone(), self.mcp.clone(), self.context.clone());

                let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
                stream
            }
        }
    }
}

#[derive(Clone)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
    pub request: ToolCallRequest,
}

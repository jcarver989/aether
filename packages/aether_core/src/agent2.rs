use crate::types::IsoString;
use crate::{llm::LlmProvider, types::ChatMessage};
use color_eyre::eyre::Result;
use tokio::sync::mpsc::Sender;

// An agent has
// N models
// A system prompt
// Memory (conversation history)
// Tools (APIs it can call)
// A way to send and receive messages (channel)

pub struct Agent2<T: LlmProvider> {
    llm: T,
    system_prompt: Option<String>,
    messages: Vec<ChatMessage>,
    tx: Sender<ChatMessage>,
}

impl<T: LlmProvider> Agent2<T> {
    pub fn new(llm: T, tx: Sender<ChatMessage>, system_prompt: Option<String>) -> Self {
        let mut messages = Vec::new();

        if let Some(system_prompt) = &system_prompt {
            messages.push(ChatMessage::System {
                content: system_prompt.clone(),
                timestamp: IsoString::now(),
            });
        }

        Agent2 {
            llm,
            tx,
            messages,
            system_prompt,
        }
    }

    pub async fn send_message(&mut self, content: &str) -> Result<()> {
        let message = ChatMessage::User {
            content: content.to_string(),
            timestamp: IsoString::now(),
        };

        self.messages.push(message);

        Ok(())
    }
}

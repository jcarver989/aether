use crate::types::{ChatMessage, LlmResponse, ToolDefinition, IsoString, ToolCallRequest};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio_stream::Stream;

// We use Box<dyn> here instead of impl Stream primarily to support a nicer user-facing API for
// alloyed models -- i.e. it allows us to have Vec<Box<dyn ModelProvider>> in AlloyedModelProvider
pub type LlmResponseStream = Pin<Box<dyn Stream<Item = Result<LlmResponse>> + Send>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    messages: Vec<ChatMessage>,
    tools: Vec<ToolDefinition>,
}

impl Context {
    pub fn new(messages: Vec<ChatMessage>, tools: Vec<ToolDefinition>) -> Self {
        Self { messages, tools }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage::User {
            content,
            timestamp: IsoString::now(),
        });
    }

    pub fn add_system_message(&mut self, content: String) {
        self.messages.push(ChatMessage::System {
            content,
            timestamp: IsoString::now(),
        });
    }

    pub fn add_assistant_message(&mut self, content: String) {
        self.messages.push(ChatMessage::Assistant {
            content,
            timestamp: IsoString::now(),
            tool_calls: Vec::new(),
        });
    }

    pub fn add_assistant_message_with_tools(&mut self, content: String, tool_calls: Vec<ToolCallRequest>) {
        self.messages.push(ChatMessage::Assistant {
            content,
            timestamp: IsoString::now(),
            tool_calls,
        });
    }

    pub fn add_tool_call_result(&mut self, tool_call_id: String, content: String) {
        self.messages.push(ChatMessage::ToolCallResult {
            tool_call_id,
            content,
            timestamp: IsoString::now(),
        });
    }

    pub fn add_tool(&mut self, tool: ToolDefinition) {
        self.tools.push(tool);
    }

    pub fn add_tools(&mut self, tools: Vec<ToolDefinition>) {
        self.tools.extend(tools);
    }

    pub fn set_tools(&mut self, tools: Vec<ToolDefinition>) {
        self.tools = tools;
    }

    pub fn messages(&self) -> &Vec<ChatMessage> {
        &self.messages
    }

    pub fn tools(&self) -> &Vec<ToolDefinition> {
        &self.tools
    }
}

pub trait ModelProvider: Send + Sync {
    fn stream_response<'a>(&self, context: &Context) -> LlmResponseStream;
    fn display_name(&self) -> String;
}

impl ModelProvider for Box<dyn ModelProvider> {
    fn stream_response<'a>(&self, context: &Context) -> LlmResponseStream {
        (**self).stream_response(context)
    }

    fn display_name(&self) -> String {
        (**self).display_name()
    }
}

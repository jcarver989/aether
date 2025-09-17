use rmcp::model::{CreateElicitationRequestParam, CreateElicitationResult};
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum AgentMessage {
    Text {
        message_id: String,
        chunk: String,
        is_complete: bool,
        model_name: String,
    },

    ToolCall {
        tool_call_id: String,
        name: String,
        arguments: Option<String>,
        result: Option<String>,
        is_complete: bool,
        model_name: String,
    },

    Error {
        message: String,
    },

    Cancelled {
        message: String,
    },

    ElicitationRequest {
        request_id: String,
        request: CreateElicitationRequestParam,
        response_sender: oneshot::Sender<CreateElicitationResult>,
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

impl From<&str> for UserMessage {
    fn from(value: &str) -> Self {
        UserMessage::Text {
            content: value.to_string(),
        }
    }
}

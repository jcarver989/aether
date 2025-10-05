use rmcp::model::CreateElicitationRequestParam;

#[derive(Debug, Clone)]
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
    },

    Done,
}

#[derive(Debug, Clone)]
pub enum UserMessage {
    Text { content: String },
    Cancel,
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

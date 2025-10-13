use serde::Serialize;

use crate::agent::AgentMessage;

pub fn agent_message(message_id: &str) -> AgentMessageBuilder {
    AgentMessageBuilder::new(message_id)
}

pub struct AgentMessageBuilder {
    message_id: String,
    model_name: String,
    chunks: Vec<AgentMessage>,
}

impl AgentMessageBuilder {
    pub fn new(message_id: &str) -> Self {
        Self {
            message_id: message_id.to_string(),
            model_name: "Fake LLM".to_string(),
            chunks: Vec::new(),
        }
    }

    pub fn text(mut self, chunks: &[&str]) -> Self {
        for chunk in chunks {
            self.chunks.push(AgentMessage::text(
                &self.message_id,
                chunk,
                false,
                &self.model_name,
            ));
        }
        self
    }

    pub fn tool_call<T: Serialize, U: Serialize>(
        mut self,
        tool_call_id: &str,
        name: &str,
        request: &T,
        result: &U,
    ) -> Self {
        let request_json = serde_json::to_string(request).expect("Failed to serialize request");
        let result_json = serde_json::to_string(result).expect("Failed to serialize result");

        // MCP wraps the result in a text content structure
        let result_json = serde_json::json!({
            "text": result_json,
            "type": "text"
        })
        .to_string();

        use crate::types::{ToolCallRequest, ToolCallResult};

        // Tool call start
        self.chunks.push(AgentMessage::ToolCall {
            request: ToolCallRequest {
                id: tool_call_id.to_string(),
                name: name.to_string(),
                arguments: String::new(),
            },
            model_name: self.model_name.clone(),
        });

        // Tool call streaming arguments
        self.chunks.push(AgentMessage::ToolCall {
            request: ToolCallRequest {
                id: tool_call_id.to_string(),
                name: String::new(),
                arguments: request_json.clone(),
            },
            model_name: self.model_name.clone(),
        });

        // Tool call streaming arguments finished
        self.chunks.push(AgentMessage::ToolCall {
            request: ToolCallRequest {
                id: tool_call_id.to_string(),
                name: name.to_string(),
                arguments: request_json.clone(),
            },
            model_name: self.model_name.clone(),
        });

        // Tool result received
        self.chunks.push(AgentMessage::ToolResult {
            result: ToolCallResult {
                id: tool_call_id.to_string(),
                name: name.to_string(),
                arguments: request_json,
                result: result_json,
            },
            model_name: self.model_name.clone(),
        });

        self
    }

    pub fn tool_call_with_error<T: Serialize>(
        mut self,
        tool_call_id: &str,
        name: &str,
        request: &T,
        error_message: &str,
    ) -> Self {
        let request_json = serde_json::to_string(request).expect("Failed to serialize request");

        // Format error like the MCP run task does
        let error_result = format!(
            "Tool execution error: Annotated {{ raw: Text(RawTextContent {{ text: \"{}\", meta: None }}), annotations: None }}",
            error_message
        );

        use crate::types::{ToolCallError, ToolCallRequest};

        // Tool call start
        self.chunks.push(AgentMessage::ToolCall {
            request: ToolCallRequest {
                id: tool_call_id.to_string(),
                name: name.to_string(),
                arguments: String::new(),
            },
            model_name: self.model_name.clone(),
        });

        // Tool call streaming arguments
        self.chunks.push(AgentMessage::ToolCall {
            request: ToolCallRequest {
                id: tool_call_id.to_string(),
                name: String::new(),
                arguments: request_json.clone(),
            },
            model_name: self.model_name.clone(),
        });

        // Tool call streaming arguments finished
        self.chunks.push(AgentMessage::ToolCall {
            request: ToolCallRequest {
                id: tool_call_id.to_string(),
                name: name.to_string(),
                arguments: request_json.clone(),
            },
            model_name: self.model_name.clone(),
        });

        self.chunks.push(AgentMessage::ToolError {
            error: ToolCallError {
                id: tool_call_id.to_string(),
                name: name.to_string(),
                arguments: Some(request_json),
                error: error_result,
            },
            model_name: self.model_name.clone(),
        });

        self
    }

    pub fn build(mut self) -> Vec<AgentMessage> {
        self.chunks.push(AgentMessage::text(
            &self.message_id,
            "",
            true,
            &self.model_name,
        ));

        self.chunks
    }
}

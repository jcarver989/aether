use crate::agent::{AgentMessage, ToolCallResult};
use crate::mcp::McpManager;
use crate::types::ToolCallRequest;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};

pub struct ToolExecutionTask {
    mcp_client: Arc<Mutex<McpManager>>,
    tx: mpsc::Sender<AgentMessage>,
    tool_call: ToolCallRequest,
    model_name: String,
    result_sender: oneshot::Sender<ToolCallResult>,
}

impl ToolExecutionTask {
    pub fn new(
        mcp_client: Arc<Mutex<McpManager>>,
        tx: mpsc::Sender<AgentMessage>,
        tool_call: ToolCallRequest,
        model_name: String,
        result_sender: oneshot::Sender<ToolCallResult>,
    ) -> Self {
        Self {
            mcp_client,
            tx,
            tool_call,
            model_name,
            result_sender,
        }
    }

    pub async fn run(self) {
        let result_str = match serde_json::from_str(&self.tool_call.arguments) {
            Ok(args) => {
                let mcp_client_guard = self.mcp_client.lock().await;
                match mcp_client_guard
                    .execute_tool(&self.tool_call.name, args)
                    .await
                {
                    Ok(result) => result.to_string(),
                    Err(e) => format!("Tool execution failed: {}", e),
                }
            }
            Err(e) => format!("Invalid tool arguments: {}", e),
        };

        // Send result for user-facing output
        let _ = self
            .tx
            .send(AgentMessage::ToolCall {
                tool_call_id: self.tool_call.id.clone(),
                name: self.tool_call.name.clone(),
                arguments: None,
                result: Some(result_str.clone()),
                is_complete: true,
                model_name: self.model_name,
            })
            .await;

        // Send result back to AgentTask for context management
        let _ = self.result_sender.send(ToolCallResult {
            id: self.tool_call.id,
            result: result_str,
        });
    }
}

use crate::agent::ToolCallResult;
use crate::mcp::McpManager;
use crate::types::ToolCallRequest;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

pub struct ToolExecutionTask {
    mcp_client: Arc<Mutex<McpManager>>,
    tool_call: ToolCallRequest,
    result_sender: mpsc::UnboundedSender<ToolCallResult>,
}

impl ToolExecutionTask {
    pub fn new(
        mcp_client: Arc<Mutex<McpManager>>,
        tool_call: ToolCallRequest,
        result_sender: mpsc::UnboundedSender<ToolCallResult>,
    ) -> Self {
        Self {
            mcp_client,
            tool_call,
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

        // Send result back to agent for context management
        let _ = self.result_sender.send(ToolCallResult {
            id: self.tool_call.id,
            name: self.tool_call.name,
            arguments: self.tool_call.arguments,
            result: result_str,
        });
    }
}

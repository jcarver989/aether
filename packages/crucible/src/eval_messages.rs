use aether::{
    agent::{AgentMessage},
    llm::ToolCallRequest,
};
use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;

/// Accumulated message types for eval logging and judging
#[derive(Debug, Clone)]
pub enum EvalMessage {
    AgentText(String),
    ToolCall { name: String, arguments: String },
    ToolResult { name: String, result: String },
    ToolError(String),
    Error(String),
    Done,
}

/// Accumulate agent messages from a receiver, yielding complete messages
pub async fn to_eval_messages(mut rx: Receiver<AgentMessage>) -> Vec<EvalMessage> {
    let mut eval_messages = Vec::new();
    let mut accumulated_text = String::new();
    let mut accumulated_tool_calls: HashMap<String, ToolCallRequest> = HashMap::new();

    while let Some(message) = rx.recv().await {
        match &message {
            AgentMessage::Text {
                chunk, is_complete, ..
            } => {
                accumulated_text.push_str(chunk);
                if *is_complete && !accumulated_text.is_empty() {
                    // Log each line separately to make grep work better
                    for line in accumulated_text.lines() {
                        tracing::info!("Agent response: {}", line);
                    }
                    eval_messages.push(EvalMessage::AgentText(accumulated_text.clone()));
                    accumulated_text.clear();
                }
            }
            AgentMessage::ToolCall { request, .. } => {
                let entry = accumulated_tool_calls
                    .entry(request.id.clone())
                    .or_insert_with(|| ToolCallRequest {
                        id: request.id.clone(),
                        name: String::new(),
                        arguments: String::new(),
                    });

                // Accumulate tool call data
                if !request.name.is_empty() {
                    entry.name.push_str(&request.name);
                }
                entry.arguments.push_str(&request.arguments);

                // Check if this is a complete tool call
                if !entry.name.is_empty() && entry.arguments.ends_with('}') {
                    tracing::info!("Tool call: {} with args: {}", entry.name, entry.arguments);
                    eval_messages.push(EvalMessage::ToolCall {
                        name: entry.name.clone(),
                        arguments: entry.arguments.clone(),
                    });
                    accumulated_tool_calls.remove(&request.id);
                }
            }
            AgentMessage::ToolResult { result, .. } => {
                tracing::info!("Tool result for {}: {}", result.name, result.result);
                eval_messages.push(EvalMessage::ToolResult {
                    name: result.name.clone(),
                    result: result.result.clone(),
                });
            }
            AgentMessage::ToolError { error, .. } => {
                tracing::info!("Tool error: {:?}", error);
                eval_messages.push(EvalMessage::ToolError(format!("{:?}", error)));
            }
            AgentMessage::Error { message: msg } => {
                tracing::info!("Agent error: {}", msg);
                eval_messages.push(EvalMessage::Error(msg.clone()));
                // Agent errors are terminal - agent won't send Done, so break out
                break;
            }
            AgentMessage::Cancelled { message: msg } => {
                tracing::info!("Agent cancelled: {}", msg);
                eval_messages.push(EvalMessage::Error(format!("Cancelled: {}", msg)));
                // Cancellation is terminal - break out
                break;
            }
            AgentMessage::Done => {
                // Log any remaining accumulated text before finishing
                if !accumulated_text.is_empty() {
                    for line in accumulated_text.lines() {
                        tracing::info!("Agent response: {}", line);
                    }
                    eval_messages.push(EvalMessage::AgentText(accumulated_text.clone()));
                    accumulated_text.clear();
                }
                tracing::info!("Agent done");
                eval_messages.push(EvalMessage::Done);
                break;
            }
        }
    }

    eval_messages
}
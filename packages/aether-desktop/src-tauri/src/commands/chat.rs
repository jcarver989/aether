use tauri::{ipc::Channel, State};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;
use uuid::Uuid;
use tokio_stream::StreamExt;
use std::time::SystemTime;
use tracing::{info, debug, error};

// Re-export types from aether_core for frontend use
pub use aether_core::{
    types::{ChatMessage, ToolDiscoveryEvent, IsoString, ToolCall, ToolCallState, StreamEvent},
    llm::provider::{StreamChunk},
};

use crate::state::AgentState;

// Tauri events for streaming and tool discovery
#[derive(Clone, Serialize, Type, Event)]
pub struct ChatStreamEvent {
    pub chunk: StreamChunk,
    pub message_id: String,
}

#[derive(Clone, Serialize, Type, Event)]
pub struct ToolDiscoveryEventWrapper {
    pub event: ToolDiscoveryEvent,
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct SendMessageRequest {
    pub content: String,
    pub message_id: Option<String>,
}

#[derive(Clone, Serialize, Type)]
pub struct SendMessageResponse {
    pub message_id: String,
}

#[tauri::command]
#[specta::specta]
pub async fn send_message(
    request: SendMessageRequest,
    events: Channel<ChatStreamEvent>,
    state: State<'_, AgentState>,
) -> Result<SendMessageResponse, String> {
    let message_id = request.message_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    
    info!("send_message called with content: '{}'", request.content);
    
    // Get agent from state
    let mut agent_guard = state.agent.lock().await;
    
    if agent_guard.is_none() {
        error!("Agent is None - not initialized");
        return Err("Agent not initialized. Please configure a provider first.".to_string());
    }
    
    info!("Agent found in state, proceeding with message");
    let agent = agent_guard.as_mut().unwrap();
    
    // Add user message to agent conversation history
    let user_message = ChatMessage::User {
        content: request.content,
        timestamp: IsoString::now(),
    };
    agent.add_message(user_message);
    
    // Start streaming response from agent
    match agent.stream_completion(None).await {
        Ok(mut stream) => {
            // Process stream chunks
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        match &chunk {
                            StreamChunk::Content { content } => {
                                // Append to agent's streaming message
                                agent.append_streaming_content(content);
                            }
                            StreamChunk::ToolCallStart { id, name } => {
                                // Start tracking tool call in agent
                                agent.active_tool_calls_mut().insert(
                                    id.clone(),
                                    aether_core::agent::PartialToolCall {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments: String::new(),
                                    },
                                );
                            }
                            StreamChunk::ToolCallArgument { id, argument } => {
                                // Append argument to partial tool call
                                if let Some(partial_call) = agent.active_tool_calls_mut().get_mut(id) {
                                    partial_call.arguments.push_str(argument);
                                }
                            }
                            StreamChunk::ToolCallComplete { id } => {
                                // Complete the tool call and add to conversation
                                if let Some(partial_call) = agent.active_tool_calls_mut().remove(id) {
                                    let tool_call_message = ChatMessage::ToolCall {
                                        id: partial_call.id.clone(),
                                        name: partial_call.name.clone(),
                                        params: partial_call.arguments.clone(),
                                        timestamp: IsoString::now(),
                                    };
                                    agent.add_message(tool_call_message);
                                }
                            }
                            StreamChunk::Done => {
                                // Finalize the streaming message
                                agent.finalize_streaming_message();
                            }
                        }
                        
                        // Send chunk to frontend
                        if let Err(e) = events.send(ChatStreamEvent {
                            chunk,
                            message_id: message_id.clone(),
                        }) {
                            eprintln!("Failed to send stream chunk: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Stream error: {}", e);
                        // Send error chunk to frontend
                        let _ = events.send(ChatStreamEvent {
                            chunk: StreamChunk::Done, // Signal end of stream
                            message_id: message_id.clone(),
                        });
                        return Err(format!("Streaming error: {}", e));
                    }
                }
            }
        }
        Err(e) => {
            return Err(format!("Failed to start streaming: {}", e));
        }
    }
    
    Ok(SendMessageResponse { message_id })
}

#[tauri::command]
#[specta::specta]
pub async fn get_chat_history(state: State<'_, AgentState>) -> Result<Vec<ChatMessage>, String> {
    let agent_guard = state.agent.lock().await;
    if let Some(agent) = agent_guard.as_ref() {
        let history = agent.conversation_history();
        Ok(history.to_vec())
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
#[specta::specta]
pub async fn clear_chat_history(state: State<'_, AgentState>) -> Result<(), String> {
    let mut agent_guard = state.agent.lock().await;
    if let Some(agent) = agent_guard.as_mut() {
        agent.clear_history();
    }
    Ok(())
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct ExecuteToolCallRequest {
    pub tool_name: String,
    pub tool_params: String, // JSON string instead of serde_json::Value
}

#[tauri::command]
#[specta::specta]
pub async fn execute_tool_call(
    request: ExecuteToolCallRequest,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let agent_guard = state.agent.lock().await;
    let agent = agent_guard.as_ref()
        .ok_or("Agent not initialized. Please configure a provider first.")?;
    
    // Parse JSON parameters
    let tool_params: serde_json::Value = serde_json::from_str(&request.tool_params)
        .map_err(|e| format!("Invalid JSON parameters: {}", e))?;
    
    // Execute the tool through the agent
    match agent.execute_tool(&request.tool_name, tool_params).await {
        Ok(result) => {
            // Convert result to string representation
            match result {
                serde_json::Value::String(s) => Ok(s),
                other => Ok(other.to_string()),
            }
        }
        Err(e) => Err(format!("Tool execution failed: {}", e)),
    }
}

// Utility commands to ensure all types are exported by tauri-specta
#[tauri::command]
#[specta::specta]
pub async fn get_tool_call_state_info() -> Result<ToolCallState, String> {
    // This is just to ensure ToolCallState is exported
    Ok(ToolCallState::Pending)
}

#[tauri::command]
#[specta::specta]
pub async fn get_tool_call_info() -> Result<ToolCall, String> {
    // This is just to ensure ToolCall is exported
    Ok(ToolCall {
        id: "example".to_string(),
        name: "example".to_string(),
        arguments: "{}".to_string(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn get_stream_event_info() -> Result<StreamEvent, String> {
    // This is just to ensure StreamEvent is exported
    Ok(StreamEvent::Done)
}
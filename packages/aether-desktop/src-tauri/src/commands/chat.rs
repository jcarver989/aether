use tauri::{ipc::Channel, State};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;
use uuid::Uuid;
use tokio_stream::StreamExt;
use tracing::{info, error};

// Re-export types from aether_core for frontend use
pub use aether_core::{
    types::{ChatMessage, ToolDiscoveryEvent, IsoString, ToolCall, ToolCallState, StreamEvent},
    llm::provider::{StreamChunk},
};

use crate::state::AgentState;

// Helper function to handle agent updates asynchronously
async fn update_agent_with_chunk(state: &AgentState, chunk: &StreamChunk) {
    let mut agent_guard = state.agent.lock().await;
    if let Some(agent) = agent_guard.as_mut() {
        match chunk {
            StreamChunk::Content { content } => {
                agent.append_streaming_content(content);
            }
            StreamChunk::ToolCallStart { id, name } => {
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
                if let Some(partial_call) = agent.active_tool_calls_mut().get_mut(id) {
                    partial_call.arguments.push_str(argument);
                }
            }
            StreamChunk::ToolCallComplete { id } => {
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
                agent.finalize_streaming_message();
            }
        }
    }
}

// The issue is that we can't avoid holding the mutex across async boundaries with this architecture.
// The cleanest solution would be to either:
// 1. Change to Arc<Agent> and avoid mutex entirely during streaming
// 2. Use message passing instead of direct mutation
// 3. Collect updates and apply them in batches
// 
// For now, let's try a more direct approach by reorganizing the main function

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
    
    // Check if agent exists and add user message
    let mut stream = {
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
        
        // Start streaming and return the stream
        agent.stream_completion(None).await.map_err(|e| e.to_string())?
    };
    
    // Process stream chunks
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                // Update agent state using helper function
                update_agent_with_chunk(&state, &chunk).await;
                
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
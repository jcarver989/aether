use tauri::{ipc::Channel, State};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;
use uuid::Uuid;

// Re-export types from aether_core for frontend use
pub use aether_core::{
    types::{ChatMessage, ToolCall, ToolCallState},
    llm::provider::{StreamChunk, ChatRequest, ToolDefinition},
};

use crate::state::AgentState;

// Wrapper for StreamChunk to make it a Tauri event
#[derive(Clone, Serialize, Type, Event)]
pub struct StreamEvent {
    pub chunk: StreamChunk,
    pub message_id: String,
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
    events: Channel<StreamEvent>,
    _state: State<'_, AgentState>,
) -> Result<SendMessageResponse, String> {
    let message_id = request.message_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    
    // TODO: Implement actual message sending with agent
    // For now, just send some mock events
    
    events.send(StreamEvent {
        chunk: StreamChunk::Content("Hello! This is a mock response.".to_string()),
        message_id: message_id.clone(),
    }).map_err(|e| format!("Failed to send event: {}", e))?;
    
    events.send(StreamEvent {
        chunk: StreamChunk::Done,
        message_id: message_id.clone(),
    }).map_err(|e| format!("Failed to send event: {}", e))?;
    
    Ok(SendMessageResponse { message_id })
}

#[tauri::command]
#[specta::specta]
pub async fn get_chat_history(_state: State<'_, AgentState>) -> Result<Vec<ChatMessage>, String> {
    // TODO: Implement actual chat history retrieval
    Ok(vec![])
}

#[tauri::command]
#[specta::specta]
pub async fn clear_chat_history(_state: State<'_, AgentState>) -> Result<(), String> {
    // TODO: Implement actual chat history clearing
    Ok(())
}
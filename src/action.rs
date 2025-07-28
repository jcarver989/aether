use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,
    // Chat-specific actions
    SubmitMessage(String),
    AddChatMessage(crate::types::ChatMessage),
    ClearChat,
    ScrollChat(ScrollDirection),
    // Input-specific actions
    ClearInput,
    // Tool call actions
    ToggleToolCall(String), // Tool call ID
    ExecuteToolCall(crate::types::ToolCall),
    UpdateToolCallState {
        id: String,
        state: crate::types::ToolCallState,
    },
    UpdateToolCallResult {
        id: String,
        result: String,
    },
    // Streaming actions
    StartStreaming,
    StreamContent(String),
    StreamToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    StreamComplete,
    // LLM Response actions (as per task requirements)
    ReceiveStreamChunk(crate::llm::StreamChunk),
    ReceiveAssistantMessage(String),
    // Tool execution results
    ToolExecutionResult {
        tool_call_id: String,
        result: String,
    },
    RefreshTools,
    // Content block interactions
    ToggleBlockExpansion(usize), // Block index
    SelectBlock(usize),          // Block index
    ToggleCodeBlockExpansion {
        block_id: usize,
        element_id: usize,
    },
    // Continue conversation after tool execution
    ContinueConversation,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum ScrollDirection {
    Up,
    Down,
    PageUp,
    PageDown,
}

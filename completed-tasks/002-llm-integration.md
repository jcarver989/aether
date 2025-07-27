# Task 002: LLM Integration

## Overview
Integrate LLM provider functionality into the action-driven architecture, enabling chat requests, streaming responses, and tool calling.

## Missing Components
The new app has LLM provider abstractions but lacks:
- Proper initialization in the main application loop
- Chat request/response handling in the action system
- Streaming response support in UI components
- Tool calling integration with MCP servers

## Requirements

### Action Integration
- Add LLM-related actions to `Action` enum:
  - `SendChatMessage(String)` - User sends message
  - `ReceiveStreamChunk(StreamChunk)` - Handle streaming response
  - `ReceiveToolCall(ToolCall)` - Handle tool call from LLM
  - `ReceiveAssistantMessage(String)` - Complete assistant response

### App State Management
- Initialize LLM provider during app startup
- Store provider in App struct (already partially done)
- Handle provider initialization errors gracefully
- Support provider switching via configuration

### Chat Request Handling
- Convert user input to `ChatRequest` format
- Include conversation history in requests
- Add agent context from AGENT.md if present
- Handle tool definitions in requests

### Streaming Response Processing
- Process `StreamChunk` events from provider
- Update UI in real-time during streaming
- Handle partial content updates
- Manage streaming state in components

### Tool Call Integration
- Parse tool calls from LLM responses
- Execute tools via MCP client
- Format tool results for LLM
- Continue conversation with tool results

## Implementation Details

### App Initialization
```rust
// In App::new()
let llm_provider = create_provider_from_env()?;
// Store in App struct
```

### Action Handling
```rust
// In App::update()
match action {
    Action::SendChatMessage(content) => {
        // Create ChatRequest with history
        // Send to LLM provider
        // Handle streaming response
    }
    Action::ReceiveStreamChunk(chunk) => {
        // Update current message
        // Trigger UI refresh
    }
    // ... other actions
}
```

### Component Updates
- Chat component shows streaming updates
- Input component triggers chat actions
- Tool call component displays tool execution

## Integration with Existing Code
- Use existing `LlmProvider` trait and implementations
- Leverage existing `ChatMessage` and `ToolDefinition` types
- Integrate with current action system architecture
- Maintain component isolation principles

## Error Handling
- Handle provider initialization failures
- Manage streaming interruptions
- Handle tool execution errors
- Display error messages in UI

## Acceptance Criteria
- [ ] LLM provider properly initialized in App
- [ ] Chat messages sent to LLM via actions
- [ ] Streaming responses displayed in real-time
- [ ] Tool calls executed and results returned
- [ ] Error scenarios handled gracefully
- [ ] Integration follows action pattern principles
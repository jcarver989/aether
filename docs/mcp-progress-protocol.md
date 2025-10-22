# MCP Progress Protocol Implementation

## Overview

This document describes Aether's implementation of the Model Context Protocol (MCP) progress tracking specification (2025-06-18). The implementation allows MCP servers to send progress updates for long-running tool executions, providing better user experience and transparency.

## Architecture

### Components

1. **ToolCallStatus Enum** (`packages/aether/src/llm/tools.rs`)
   - Represents the lifecycle states of a tool call
   - Variants:
     - `Started`: Tool execution has begun
     - `InProgress`: Tool is executing with progress updates
     - `Complete`: Tool finished successfully
     - `Error`: Tool execution failed

2. **Status Streaming Channel** (`packages/aether/src/mcp/run_mcp_task.rs`)
   - Changed from `oneshot::channel` to `mpsc::channel` for `ExecuteTool` command
   - Allows multiple status updates per tool call
   - Channel capacity: 10 messages

3. **Agent Stream Processing** (`packages/aether/src/agent/core.rs`)
   - Converts status stream into agent events
   - Filters intermediate progress updates (currently)
   - Emits final result or error to the agent

4. **Progress Token Management** (`packages/aether/src/mcp/run_mcp_task.rs`)
   - Uses tool call ID as the progress token
   - Ensures uniqueness across active requests

## Implementation Status

### ✅ Completed

1. **Data Structures**
   - `ToolCallStatus` enum with all states (Started, InProgress, Complete, Error)
   - `ToolCallProgress` struct for progress information (progress, total, message)
   - Comprehensive unit tests for all variants

2. **Channel Infrastructure**
   - `McpCommand::ExecuteTool` now uses `mpsc::Sender<ToolCallStatus>`
   - Agent creates `ReceiverStream` to process status updates
   - Proper timeout handling maintained
   - Progress channel registration and cleanup

3. **Progress Notification Routing**
   - `ProgressChannelMap` shared across MCP manager and clients
   - `McpClient::on_progress()` method implemented
   - Notifications routed by progress token to correct tool execution
   - Automatic cleanup when tool execution completes

4. **Status Flow**
   - MCP manager sends `Started` status when tool execution begins
   - Progress notifications from MCP servers → `InProgress` status
   - MCP manager sends `Complete` or `Error` status when tool finishes
   - Agent processes these statuses and converts to tool results

### 🚧 In Progress / TODO

1. **Progress Token Injection into MCP Requests**
   - **Status**: Infrastructure ready, need rmcp API support
   - **Location**: `packages/aether/src/mcp/run_mcp_task.rs:112-130`
   - **What's needed**:
     ```rust
     // Need to inject into MCP request metadata:
     // According to MCP spec, this goes in the JSON-RPC params._meta field
     {
       "jsonrpc": "2.0",
       "method": "tools/call",
       "params": {
         "_meta": {
           "progressToken": tool_call_id
         },
         ...tool_params
       }
     }
     ```
   - **Current approach**:
     - Progress token is generated (tool call ID)
     - Progress channel is registered before execution
     - Token is passed to `try_execute_tool()`
     - TODO: Inject into actual MCP request

   - **Potential solutions**:
     - Use rmcp's lower-level request building API
     - Extend `CallToolRequestParam` with extensions/meta
     - Use rmcp's `GetMeta` trait if available on requests
     - Contribute to rmcp to add `call_tool_with_progress()` method

2. **Testing with Real MCP Servers**
   - Create integration tests with mock MCP server that sends progress
   - Verify end-to-end flow works correctly
   - Test edge cases (late notifications, multiple tools, etc.)

## Data Flow

### Current Implementation

```
User Request
    ↓
Agent receives tool call from LLM
    ↓
Agent sends McpCommand::ExecuteTool { request, tx: mpsc::Sender<ToolCallStatus> }
    ↓
MCP Manager receives command
    ↓
MCP Manager sends ToolCallStatus::Started
    ↓
MCP Manager executes tool via client.call_tool()
    ↓
MCP Manager sends ToolCallStatus::Complete or ToolCallStatus::Error
    ↓
Agent receives status via stream
    ↓
Agent converts to ToolResult and continues processing
```

### Future with Full Progress Support

```
User Request
    ↓
Agent receives tool call from LLM
    ↓
Agent sends McpCommand::ExecuteTool with progress channel
    ↓
MCP Manager generates unique progress token (tool call ID)
    ↓
MCP Manager injects progress token into request._meta
    ↓
MCP Manager sends ToolCallStatus::Started
    ↓
MCP Manager calls client.call_tool() with metadata
    ↓
    ├─→ MCP Server executes tool (async)
    ├─→ MCP Server sends notifications/progress periodically
    ├─→ MCP Client receives progress notifications
    ├─→ MCP Client routes to correct status channel by token
    ├─→ Agent receives ToolCallStatus::InProgress updates
    └─→ MCP Server completes and returns result
    ↓
MCP Manager sends ToolCallStatus::Complete or Error
    ↓
Agent processes final result
```

## MCP Specification Reference

According to [MCP Progress Spec (2025-06-18)](https://modelcontextprotocol.io/specification/2025-06-18/basic/utilities/progress):

### Request with Progress Token

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "_meta": {
      "progressToken": "abc123"
    },
    "name": "long_running_tool",
    "arguments": {...}
  }
}
```

### Progress Notification

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/progress",
  "params": {
    "progressToken": "abc123",
    "progress": 50,
    "total": 100,
    "message": "Processing items..."
  }
}
```

### Requirements

- Progress tokens **MUST** be string or integer
- Progress tokens **MUST** be unique across active requests
- Progress value **MUST** increase with each notification
- Progress and total **MAY** be floating point
- Servers **MAY** omit total if unknown
- Servers **MAY** choose not to send progress notifications

## Testing Strategy

### Unit Tests

1. **ToolCallStatus Serialization**
   - Test serde serialization/deserialization
   - Verify JSON format matches expectations

2. **Channel Communication**
   - Test sending multiple status updates
   - Verify channel buffering behavior
   - Test channel closure handling

### Integration Tests

1. **End-to-End Tool Execution**
   - Mock MCP server that sends progress notifications
   - Verify status updates flow through system
   - Test error handling and timeouts

2. **Progress Token Uniqueness**
   - Execute multiple tools concurrently
   - Verify each gets unique progress token
   - Verify notifications route to correct tool

## Migration Guide

### For Existing Code

No breaking changes for existing code! The implementation is backward compatible:

- Old code using tool results will continue to work
- Progress updates are optional enhancements
- Existing MCP servers without progress support work unchanged

### For New Features

To leverage progress updates:

1. **In Agent Logic**:
   ```rust
   // Tool execution automatically handles status updates
   // No changes needed - progress happens transparently
   ```

2. **In UI/TUI** (future):
   ```rust
   match agent_message {
       AgentMessage::ToolCall { request, .. } => {
           // Tool started - show spinner
       }
       AgentMessage::ToolProgress { progress, .. } => {
           // Update progress bar (when implemented)
       }
       AgentMessage::ToolResult { result, .. } => {
           // Tool completed - show result
       }
   }
   ```

## Next Steps

1. **Coordinate with rmcp maintainers**
   - Request API for injecting request metadata
   - Or: Request `call_tool_with_progress()` method
   - Or: Implement using lower-level rmcp APIs

2. **Implement progress notification routing**
   - Add notification handler to `McpClient`
   - Create mapping of progress tokens to channels
   - Forward notifications to correct channel

3. **Add progress to AgentMessage**
   - New variant: `ToolProgress { id, progress }`
   - Allow UIs to show progress bars
   - Optional feature for backwards compatibility

4. **Create integration tests**
   - Mock MCP server with progress notifications
   - Verify end-to-end flow
   - Test edge cases (timeout, errors, etc.)

## References

- [MCP Progress Specification](https://modelcontextprotocol.io/specification/2025-06-18/basic/utilities/progress)
- [rmcp Crate Documentation](https://docs.rs/rmcp)
- [Aether Architecture](../CLAUDE.md)

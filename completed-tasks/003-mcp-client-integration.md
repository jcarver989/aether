# Task 003: MCP Client Integration

## Overview
Integrate MCP (Model Context Protocol) client functionality into the action-driven architecture to enable dynamic tool discovery and execution.

## Missing Components
The new app has MCP code but lacks:
- MCP server connections during startup
- Tool loading from mcp.json configuration
- Tool execution during conversations
- Integration with the action system

## Requirements

### App Startup Integration
- Initialize MCP client during app startup
- Load server configurations from mcp.json
- Connect to configured MCP servers
- Discover available tools from all servers
- Handle connection failures gracefully

### Action System Integration
- Add MCP-related actions to `Action` enum:
  - `ExecuteTool { name: String, params: serde_json::Value }` - Execute a tool
  - `ToolExecutionResult { result: String }` - Tool execution completed
  - `ToolExecutionError { error: String }` - Tool execution failed
  - `RefreshTools` - Rediscover tools from servers

### Tool Execution Workflow
- Receive tool calls from LLM responses
- Validate tool exists in registry
- Execute tool via appropriate MCP server
- Format results for LLM consumption
- Handle execution errors and timeouts

### Configuration Loading
- Read mcp.json from project root
- Parse server configurations
- Support different transport types (stdio, http)
- Validate server configurations

## Implementation Details

### App Initialization
```rust
// In App::new()
let mut mcp_client = McpClient::new();
let config = load_mcp_config()?;
for (name, server_config) in config.servers {
    mcp_client.connect_server(name, server_config).await?;
}
mcp_client.discover_tools().await?;
```

### Action Handling
```rust
// In App::update()
match action {
    Action::ExecuteTool { name, params } => {
        // Execute tool via MCP client
        // Handle async execution
        // Return result or error action
    }
    Action::ToolExecutionResult { result } => {
        // Add result to conversation
        // Continue LLM processing
    }
    // ... other actions
}
```

### Tool Call Processing
- Parse tool calls from LLM streaming responses
- Convert to ExecuteTool actions
- Handle multiple concurrent tool executions
- Aggregate results for LLM context

### Error Handling
- Server connection failures
- Tool execution timeouts
- Invalid tool parameters
- Server disconnections
- Tool not found errors

## Integration Points

### With LLM Integration (Task 002)
- Tool definitions sent to LLM in chat requests
- Tool calls parsed from LLM responses
- Tool results included in conversation history

### With Configuration (Task 004)
- MCP server configurations loaded from config
- Environment variables for server settings
- Runtime configuration updates

### With UI Components
- Tool call component displays execution status
- Error messages shown in chat
- Tool availability in UI indicators

## File Structure
```
src/mcp/
├── client.rs     # MCP client implementation
├── protocol.rs   # JSON-RPC protocol handling  
├── registry.rs   # Tool registry management
└── mod.rs        # Module exports
```

## Configuration Format
```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allowed/files"]
    },
    "git": {
      "command": "uvx",
      "args": ["mcp-server-git", "--repository", "/path/to/repo"]
    }
  }
}
```

## Acceptance Criteria
- [ ] MCP client initialized during app startup
- [ ] Servers connected from mcp.json configuration
- [ ] Tools discovered and registered successfully
- [ ] Tool execution via action system works
- [ ] Tool results properly formatted for LLM
- [ ] Error handling for all failure scenarios
- [ ] Integration follows action pattern principles
- [ ] Multiple concurrent tool executions supported
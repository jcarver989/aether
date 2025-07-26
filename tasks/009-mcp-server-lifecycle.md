# Task 009: MCP Server Lifecycle Management

## Objective
Implement robust lifecycle management for MCP server processes.

## Requirements
1. In `src/mcp/mod.rs`, implement connection manager:
   - Spawn multiple MCP servers concurrently
   - Initialize each server with proper handshake
   - Discover and aggregate tools from all servers
   - Monitor server health
   - Graceful shutdown on application exit

2. Server management features:
   ```rust
   pub struct McpManager {
       servers: HashMap<String, McpServer>,
       registry: ToolRegistry,
   }
   
   pub struct McpServer {
       id: String,
       process: Child,
       client: McpClient,
       status: ServerStatus,
   }
   ```

3. Implement methods:
   - start_servers: Launch all configured MCP servers
   - discover_tools: Aggregate tools from all servers
   - route_tool_call: Send tool request to correct server
   - shutdown_all: Clean termination of all servers

## Deliverables
- Complete MCP manager implementation
- Concurrent server initialization
- Proper process cleanup on drop
- Health monitoring for servers
- Integration with main application

## Notes
- Handle server crashes gracefully
- Implement timeout for initialization
- Log server stdout/stderr for debugging
- Ensure zombie processes are avoided
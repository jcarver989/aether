# Task 003: MCP Protocol Core Implementation

## Objective
Integrate the official MCP Rust SDK (rmcp) to implement MCP client functionality.

## Requirements
1. Add rmcp dependency to Cargo.toml:
   - Add `rmcp = "0.2.0"` with appropriate features
   - Consider using git dependency for latest features if needed

2. In `src/mcp/client.rs`, implement:
   - MCP client using rmcp::ServiceExt trait
   - Support for TokioChildProcess transport for stdio-based communication
   - Connection management using rmcp's built-in lifecycle handling
   - Wrapper around rmcp client for our application's needs

3. In `src/mcp/protocol.rs`, implement:
   - Re-export necessary rmcp types and traits
   - Application-specific error handling and result types
   - Helper functions for common MCP operations

4. Core functionality to implement:
   - Client initialization with MCP servers from mcp.json
   - Tool discovery using service.list_tools()
   - Tool execution using service.call_tool()
   - Proper error handling and logging

## Deliverables
- rmcp integration in Cargo.toml
- MCP client wrapper using rmcp SDK
- Protocol abstractions for application use
- Integration tests with real MCP servers (filesystem, git)

## Notes
- Use rmcp::transport::TokioChildProcess for spawning server processes
- Leverage rmcp's built-in JSON-RPC 2.0 and MCP protocol implementation
- Focus on application-specific logic rather than protocol details
- Handle rmcp errors and convert to application error types
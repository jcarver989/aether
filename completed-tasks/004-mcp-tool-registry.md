# Task 004: MCP Tool Registry

## Objective
Implement a tool registry that manages discovered tools from multiple MCP servers.

## Requirements
1. In `src/mcp/registry.rs`, implement:
   - Tool registry that stores tool definitions from all connected MCP servers
   - Mapping between tool names and their source MCP servers
   - Tool schema validation and storage
   
2. Create the following types:
   ```rust
   pub struct ToolRegistry {
       tools: HashMap<String, ToolDefinition>,
       tool_to_server: HashMap<String, String>,
   }
   
   pub struct ToolDefinition {
       pub name: String,
       pub description: String,
       pub input_schema: serde_json::Value,
       pub server_id: String,
   }
   ```

3. Implement methods:
   - register_tools: Add tools from an MCP server
   - get_tool: Retrieve tool definition by name
   - list_tools: Get all available tools
   - execute_tool: Route tool execution to correct MCP server

## Deliverables
- Complete tool registry implementation
- Tool discovery integration with MCP client
- Proper handling of tool name conflicts
- Unit tests for registry operations

## Notes
- Consider tool namespacing if multiple servers provide same tool
- Validate tool schemas on registration
- Provide clear error messages for missing tools
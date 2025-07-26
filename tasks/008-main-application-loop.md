# Task 008: Main Application Loop

## Objective
Implement the main application entry point that ties all components together.

## Requirements
1. In `src/main.rs`, implement:
   - Application initialization sequence
   - Load configuration from mcp.json
   - Read AGENT.md if present
   - Initialize MCP servers based on config
   - Create LLM provider instance
   - Start Ratatui UI
   - Main conversation loop

2. Conversation flow:
   - Accept user input from UI
   - Build chat context with AGENT.md content
   - Send to LLM with available tools
   - Handle tool calls through MCP
   - Display responses in UI
   - Continue until exit

3. Error handling:
   - Display configuration errors clearly
   - Show MCP connection failures
   - Handle LLM API errors gracefully
   - Ensure terminal cleanup on panic

## Deliverables
- Complete main.rs implementation
- Proper initialization sequence
- Working conversation loop
- Clean error display in UI
- Graceful shutdown handling

## Notes
- Use tokio::main for async runtime
- Inject AGENT.md content into system prompt
- Show startup progress in UI
- Consider startup diagnostics for debugging
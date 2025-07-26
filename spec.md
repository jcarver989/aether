# Aether - Claude Code-esque Coding Agent Specification

## Overview

Aether is a terminal-based AI coding assistant written in Rust that provides Claude Code-like functionality through a modular architecture. Unlike traditional coding agents with baked-in tools, Aether leverages the Model Context Protocol (MCP) for dynamic tool discovery and integration, supporting both OpenRouter and Ollama as LLM providers.

## Core Architecture

### Technology Stack
- **Language**: Rust
- **LLM Integration**: async-openai crate (OpenAI-compatible API)
- **MCP Client**: Custom Rust implementation of Model Context Protocol
- **UI Framework**: Ratatui for terminal UI
- **Async Runtime**: Tokio

### Key Design Principles
1. **Tool Agnostic**: All tools provided via MCP servers, no hard-coded capabilities
2. **Provider Flexible**: Support for OpenRouter and Ollama through OpenAI-compatible endpoints
3. **Simple Configuration**: Standard mcp.json format for MCP server configuration
4. **Minimal MVP**: Focus on core functionality, fail fast with clear error messages

## Components

### 1. Main Application (`src/main.rs`)
- Initialize Ratatui TUI
- Load configuration (mcp.json, environment variables)
- Read AGENT.md if present in working directory
- Start async runtime
- Handle keyboard input (Ctrl+C for interrupt)

### 2. MCP Client (`src/mcp/`)
- **Connection Manager**: Handle connections to multiple MCP servers
- **Protocol Implementation**: JSON-RPC 2.0 over stdio/SSE
- **Tool Registry**: Dynamic tool discovery and registration
- **Message Handler**: Route MCP messages between LLM and servers

### 3. LLM Provider (`src/llm/`)
- **Provider Interface**: Abstract interface for LLM interactions
- **OpenRouter Client**: Implementation using async-openai
- **Ollama Client**: Implementation using async-openai with local endpoint
- **Response Streaming**: Handle streaming responses from LLMs

### 4. UI Components (`src/ui/`)
- **Chat View**: Scrollable log-style display of conversation
- **Tool Call Widget**: Visual representation of tool invocations
- **Result Display**: Formatted display of tool results
- **Input Handler**: User input with proper key handling

### 5. Configuration (`src/config/`)
- **Config Loader**: Parse mcp.json and environment variables
- **Model Settings**: Default models for each provider
- **Provider Selection**: Logic to choose between OpenRouter/Ollama

## Configuration Files

### mcp.json
```json
{
  "mcpServers": {
    "filesystem": {
      "command": "mcp-server-filesystem",
      "args": ["--root", "."],
      "env": {}
    },
    "git": {
      "command": "mcp-server-git",
      "args": [],
      "env": {}
    }
  }
}
```

### AGENT.md
Optional file in working directory that gets injected into system prompt, allowing users to provide context about their project, coding standards, or specific instructions.

### Environment Variables
- `OPENROUTER_API_KEY`: API key for OpenRouter
- `OLLAMA_BASE_URL`: Base URL for Ollama (default: http://localhost:11434)
- `DEFAULT_PROVIDER`: "openrouter" or "ollama" (default: openrouter)
- `DEFAULT_MODEL`: Model name (provider-specific)

## User Interaction Flow

1. **Startup**
   - Load mcp.json configuration
   - Connect to configured MCP servers
   - Discover available tools
   - Initialize LLM provider
   - Display welcome message

2. **Conversation Loop**
   - Accept user input
   - Send to LLM with available tools
   - Handle tool calls via MCP
   - Display responses in scrollable log
   - Continue until exit

3. **Tool Execution**
   - LLM requests tool use
   - Forward to appropriate MCP server
   - Receive and parse results
   - Return to LLM for processing

## Error Handling

For MVP, implement simple fail-fast error handling:
- Display clear error messages to user
- Exit gracefully on critical failures
- No retry logic or recovery mechanisms

## Project Structure

```
aether/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── config/
│   │   └── mod.rs
│   ├── llm/
│   │   ├── mod.rs
│   │   ├── provider.rs
│   │   ├── openrouter.rs
│   │   └── ollama.rs
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── protocol.rs
│   │   └── registry.rs
│   └── ui/
│       ├── mod.rs
│       ├── app.rs
│       ├── widgets/
│       │   ├── chat.rs
│       │   ├── tool_call.rs
│       │   └── input.rs
│       └── event.rs
├── mcp.json (example)
└── AGENT.md (optional, user-provided)
```

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
async-openai = "0.x"
ratatui = "0.x"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = "4"
anyhow = "1"
crossterm = "0.x"
```

## MVP Deliverables

1. Basic MCP client implementation
2. OpenRouter/Ollama integration via async-openai
3. Ratatui-based TUI with scrollable chat
4. Tool discovery and execution
5. AGENT.md context injection
6. Simple error display

## Future Enhancements (Post-MVP)

- Conversation history/persistence
- Multiple concurrent MCP connections
- Advanced error handling and retries
- Tool result caching
- Configuration hot-reloading
- Debug/verbose logging modes
- Custom themes for TUI
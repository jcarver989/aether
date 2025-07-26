# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Aether is a terminal-based AI coding assistant written in Rust that provides Claude Code-like functionality through a modular architecture. It leverages the Model Context Protocol (MCP) for dynamic tool discovery and integration, supporting both OpenRouter and Ollama as LLM providers.

## Build and Development Commands

```bash
# Build the project
cargo build

# Run the project
cargo run

# Run tests
cargo test

# Run with release optimizations
cargo build --release
cargo run --release

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Architecture

The codebase follows a modular architecture with clear separation of concerns:

- **MCP Integration** (`src/mcp/`): Handles Model Context Protocol client implementation
  - `client.rs`: Connection manager for MCP servers
  - `protocol.rs`: JSON-RPC 2.0 protocol implementation
  - `registry.rs`: Tool registry for dynamic tool discovery

- **LLM Providers** (`src/llm/`): Abstracts LLM interactions
  - `provider.rs`: Common provider interface
  - `openrouter.rs`: OpenRouter implementation using async-openai
  - `ollama.rs`: Ollama implementation for local models

- **Terminal UI** (`src/ui/`): Ratatui-based terminal interface
  - `app.rs`: Main application state and rendering
  - `event.rs`: Keyboard and event handling
  - `widgets/`: UI components (chat view, input, tool calls)

- **Configuration** (`src/config/`): Handles mcp.json and environment variables

## Key Design Principles

1. **Tool Agnostic**: All tools are provided via MCP servers - no hard-coded capabilities
2. **Provider Flexible**: Supports multiple LLM providers through a common interface
3. **Fail Fast**: MVP focuses on clear error messages rather than recovery mechanisms

## Configuration

The project uses:
- `mcp.json`: Configures MCP servers (filesystem, git, etc.)
- Environment variables:
  - `OPENROUTER_API_KEY`
  - `OLLAMA_BASE_URL` (default: http://localhost:11434)
  - `DEFAULT_PROVIDER`: "openrouter" or "ollama"
  - `DEFAULT_MODEL`: Provider-specific model name

## Current State

The project is in early development. The main.rs currently contains a placeholder implementation. The full architecture is specified in `spec.md` with the following components ready to be implemented:
- MCP client for tool integration
- LLM provider abstraction
- Ratatui-based terminal UI
- Configuration management

## Development Notes

- Uses Tokio for async runtime
- async-openai crate provides OpenAI-compatible API interface
- Ratatui handles terminal UI rendering
- All external tools come from MCP servers configured in mcp.json

### Testing

1. Ensure you write tests for the code you write. 
2. Tests should be placed in a `tests/` directory, following Rust conventions for integration testing. 
3. When writing tests, use "real" implementations where possible. When you need a fake, avoid using mocks and instead create in-memory Fake implementations that can be passed into tests.
4. Tests should be thread safe and create the data they need + scaffolding for each test.
5. Tests inherit all the best practices that apply to "normal code" -- e.g. DRY, create helper methods to cut down on verbosity etc.
6. Ensure all tests pass before moving onto something else.
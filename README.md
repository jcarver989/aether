# Aether - Claude Code-esque Coding Agent

A terminal-based AI coding assistant written in Rust that provides Claude Code-like functionality through a modular architecture using the Model Context Protocol (MCP).

## Project Status

This is an MVP implementation with basic structure in place. All core modules have been created with stub implementations (`todo!()` macros) that need to be filled in.

## Project Structure

```
aether/
├── Cargo.toml          # Dependencies configured
├── src/
│   ├── main.rs        # Application entry point
│   ├── config/        # Configuration management
│   ├── llm/          # LLM provider implementations
│   ├── mcp/          # Model Context Protocol client
│   └── ui/           # Ratatui-based terminal UI
├── mcp.json          # Example MCP server configuration
└── .gitignore        # Git ignore rules
```

## Dependencies

- **tokio** - Async runtime
- **async-openai** - OpenAI-compatible API client
- **ratatui** - Terminal UI framework
- **serde/serde_json** - Serialization
- **clap** - CLI argument parsing
- **anyhow** - Error handling
- **crossterm** - Terminal manipulation

## Configuration

### Environment Variables

- `OPENROUTER_API_KEY` - API key for OpenRouter (required for OpenRouter provider)
- `OLLAMA_BASE_URL` - Base URL for Ollama (default: http://localhost:11434)
- `DEFAULT_PROVIDER` - Default LLM provider: "openrouter" or "ollama" (default: openrouter)
- `DEFAULT_MODEL` - Default model name (provider-specific)

### MCP Configuration

Edit `mcp.json` to configure MCP servers:

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

Create an optional `AGENT.md` file in your working directory to provide project-specific context and instructions.

## Building

```bash
cargo build
```

## Running

```bash
# With OpenRouter
export OPENROUTER_API_KEY=your_api_key
cargo run

# With Ollama
cargo run -- --provider ollama --model llama2
```

## Implementation Status

- ✅ Project structure created
- ✅ Dependencies configured
- ✅ Module stubs created
- ⏳ MCP client implementation needed
- ⏳ LLM provider implementations needed
- ⏳ UI implementation needed
- ⏳ Configuration loading needed

## Next Steps

1. Implement configuration loading from `mcp.json`
2. Implement MCP client for server communication
3. Implement OpenRouter and Ollama providers
4. Build the Ratatui UI components
5. Wire everything together in the main application loop
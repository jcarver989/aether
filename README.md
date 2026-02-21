# Aether

A modular Rust toolkit for building AI agents (LLM + prompt + tools + loop). 

## Use Cases

This repo contains several crates that can be used indivdually or combined like Voltron to get a fully-fleged open-source coding assistant.

### Talk to any LLM

[`llm`](packages/llm) is a unified streaming interface across LLM providers. Anthropic, OpenRouter, OpenAI, DeepSeek, Gemini, ZAI, Ollama, and Llama.cpp are supported out of the box. Or implement the `StreamableModelProvider` trait to add your own. You can also alloy models together — round-robin across providers per turn to combine their strengths.

### Build a custom agent

[`aether`](packages/aether) is the core crate. Create a custom agent in ~10 lines of Rust and tailor it to your domain. Aether agents start as a blank slate with no system prompt or tools, so you control every token in your agent's context window.

Agents get their tools from [MCP](https://modelcontextprotocol.io/) servers — write tool servers in any language and connect them via a standard `mcp.json` file. This repo includes several pre-built servers to get you started:

- [`mcp-coding`](packages/mcp-coding) — file operations, bash, and LSP integration
- [`mcp-subagents`](packages/mcp-subagents) — spawn and orchestrate sub-agents
- [`mcp-tasks`](packages/mcp-tasks) — task tracking and management
- [`mcp-skills`](packages/mcp-skills) — slash commands and reusable skills

### Run your agent in an interactive terminal

[`wisp`](packages/wisp) is a TUI (terminal UI) for Aether agents (it also works with any [ACP](https://agentclientprotocol.com/get-started/introduction) compatible agent).

### Connect your agent to an IDE or UI

[`aether-acp`](packages/aether-acp) Connect your agent to any [ACP](https://agentclientprotocol.com/get-started/introduction) compatible client ([see list](https://agentclientprotocol.com/get-started/clients)).

### Run a fully-fledged, open source coding agent

Combine all the above for a "batteries-included" AI coding agent: [`wisp`](packages/wisp) (TUI) + [`aether-acp`](packages/aether-acp) (ACP server) + the pre-built MCP tool servers for [file ops & bash](packages/mcp-coding), [sub-agents](packages/mcp-subagents), [tasks](packages/mcp-tasks), and [slash commands](packages/mcp-skills). 

See each package's README for detailed usage: [`aether`](packages/aether), [`llm`](packages/llm), [`wisp`](packages/wisp), [`aether-acp`](packages/aether-acp).

## Quick Start

Run the full coding agent (Voltron mode):

```bash
# Interactive TUI
cargo run -p wisp

# One-shot prompt
cargo run -p wisp -- "Explain this codebase"

# Headless ACP server (for editor integration)
cargo run -p aether-acp -- --model anthropic:claude-sonnet-4-20250514 --mcp-config mcp.json
```

## Development

Standard cargo workflow: `cargo check`, `cargo test`, `cargo fmt`, `cargo clippy`.

## License

MIT

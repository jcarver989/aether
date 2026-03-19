# aether-cli

Binary package containing Aether's two runnable entrypoints:

- **`aether-acp`** — [Agent Client Protocol (ACP)](https://agentclientprotocol.com/overview/introduction) server for editor/IDE integration (e.g. Zed)
- **`aether`** — Headless CLI for single-prompt usage

## Quick Start

### Build

From the workspace root:

```bash
cargo build --release -p aether-cli
```

Binaries will be at `target/release/aether-acp` and `target/release/aether`.

### Run the CLI

```bash
cargo run -p aether-cli --bin aether -- -m anthropic:claude-sonnet-4-20250514 "Refactor auth module"
```

### Run the ACP server

```bash
cargo run -p aether-cli --bin aether-acp -- --model anthropic:claude-sonnet-4-20250514 --mcp-config mcp.json
```

## Choosing a Model

Aether supports multiple LLM providers using a `provider:model` string format:

| Provider | Example | Env var required |
|----------|---------|-----------------|
| Anthropic | `anthropic:claude-sonnet-4-5-20250929` | `ANTHROPIC_API_KEY` |
| OpenRouter | `openrouter:moonshotai/kimi-k2-thinking` | `OPENROUTER_API_KEY` |
| ZAI | `zai:GLM-4.6` | `ZAI_API_KEY` |
| Ollama | `ollama:llama3.2` | None (local) |
| Llama.cpp | `llamacpp` | None (local) |

## Editor Integration (ACP)

### Zed

Add to your Zed `settings.json` (Main Menu -> "Open Settings File"):

```json
{
  "agent_servers": {
    "Aether Agent": {
      "command": "/path/to/aether/target/release/aether-acp",
      "args": [
        "--model",
        "zai:GLM-4.6",
        "--mcp-config",
        "/path/to/aether/mcp.json"
      ],
      "env": {
        "RUST_LOG": "debug",
        "ZAI_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

Then open the [Agent Panel](https://zed.dev/docs/ai/agent-panel) and select "New Aether Agent Thread".

**Important:** Update the paths and configuration:
- `command`: Full path to your built `aether-acp` binary
- `--mcp-config`: Path to your MCP configuration file
- Set the appropriate API key env var for your model provider

## MCP Configuration

The `mcp.json` file configures MCP tool servers:

```json
{
  "servers": {
    "coding": {
      "type": "in-memory"
    },
    "plugins": {
      "type": "in-memory",
      "args": ["--dir", "$HOME/.aether"]
    }
  }
}
```

- **coding** — Filesystem tools (read, write, bash, etc.)
- **plugins** — Custom slash commands from `~/.aether/commands/`

## Slash Commands

Create markdown files in `~/.aether/commands/` to define custom slash commands.

**Example** `~/.aether/commands/plan.md`:

```markdown
---
description: Create a detailed implementation spec for a task
---

You are an expert software architect. Create a comprehensive technical specification.

# Task
$ARGUMENTS
```

**Parameter syntax:**
- `$ARGUMENTS` — Full argument string (e.g., `/plan add user auth` -> "add user auth")
- `$1`, `$2`, `$3` — Positional arguments

## Settings

Project-level agent configuration is centralized in `.aether/settings.json` in your project root. This file defines agents (modes and sub-agents), prompts, and MCP server configuration.

### Agents (Modes and Sub-agents)

Define agents with specific model, prompts, and tool configurations:

```json
{
  "prompts": ["SYSTEM.md", "AGENTS.md"],
  "mcpServers": ".aether/mcp/default.json",
  "agents": [
    {
      "name": "planner",
      "description": "Planner optimized for decomposition and sequencing",
      "model": "anthropic:claude-sonnet-4-5",
      "reasoningEffort": "high",
      "userInvocable": true,
      "agentInvocable": true,
      "prompts": [".aether/prompts/planner.md"],
      "mcpServers": ".aether/mcp/planner.json"
    },
    {
      "name": "researcher",
      "description": "Read-only research agent",
      "model": "anthropic:claude-sonnet-4-5",
      "userInvocable": false,
      "agentInvocable": true,
      "prompts": [".aether/prompts/researcher.md"],
      "tools": {
        "allow": ["coding__grep", "coding__read_file", "coding__glob"],
        "deny": []
      }
    },
    {
      "name": "coder",
      "description": "Fast coding agent",
      "model": "deepseek:deepseek-chat",
      "userInvocable": true,
      "agentInvocable": false,
      "prompts": [".aether/prompts/coder.md"]
    }
  ]
}
```

- **`userInvocable: true`** — Agent appears as a mode option in ACP clients (e.g., Wisp's Shift+Tab)
- **`agentInvocable: true`** — Agent can be spawned as a sub-agent
- **`prompts`** — Explicit prompt file references (supports glob patterns)
- **`mcpServers`** — Path to MCP configuration file (optional, overrides top-level `mcpServers`)
- **`tools`** — Filter which MCP tools the agent can use (optional). Supports `allow` (allowlist) and `deny` (blocklist) with trailing `*` wildcards. If both are set, `allow` is applied first, then `deny` removes from the result. Omit or leave empty to allow all tools.
- Top-level `prompts` are inherited by all agents
- Top-level `mcpServers` is the default MCP config for all agents

## Logs

Logs are written to `--log-dir` (default: `/tmp/aether-acp-logs/`). Control verbosity with the `RUST_LOG` environment variable.

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

Aether stores its settings in `~/.aether/settings.json` (override with `AETHER_HOME` env var). The file is created with defaults on first run.

### Modes

Define named modes to quickly switch between model + reasoning configurations:

```json
{
  "modes": {
    "Planner": { "model": "anthropic:claude-opus-4-6", "reasoningEffort": "high" },
    "Coder": { "model": "deepseek:deepseek-chat" }
  }
}
```

- Mode names appear as an ACP `mode` config option alongside the existing `Model` and `Reasoning Effort` options.
- Selecting a mode updates the model and reasoning effort for the next prompt.
- ACP clients can cycle modes with Shift+Tab only when the option is emitted as `SessionConfigOptionCategory::Mode` (e.g. Wisp uses this category to detect cycleable mode options).
- Invalid modes (unknown model or invalid reasoning effort) are silently skipped.

## Logs

Logs are written to `--log-dir` (default: `/tmp/aether-acp-logs/`). Control verbosity with the `RUST_LOG` environment variable.

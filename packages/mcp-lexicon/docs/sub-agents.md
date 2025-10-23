# Sub-Agents

Sub-agents allow you to spawn specialized AI agents with their own system prompts and MCP configurations to handle specific tasks.

## Directory Structure

Sub-agents are defined in the `sub-agents/` directory within your base plugin directory (typically `~/.aether/sub-agents/`).

Each sub-agent has its own directory containing:
- `AGENTS.md` - Agent definition with frontmatter and system prompt
- `mcp.json` (optional) - MCP server configuration for the agent

Example structure:
```
~/.aether/
└── sub-agents/
    ├── debugger/
    │   ├── AGENTS.md
    │   └── mcp.json
    ├── code-reviewer/
    │   ├── AGENTS.md
    │   └── mcp.json
    └── data-analyst/
        └── AGENTS.md
```

## Agent Definition (AGENTS.md)

The `AGENTS.md` file contains YAML frontmatter with agent metadata and the agent's system prompt:

```markdown
---
description: Debug and fix code issues
model: anthropic:claude-3.5-sonnet
---

You are an expert debugging assistant. Your role is to:
- Analyze code for bugs and errors
- Suggest fixes with clear explanations
- Use debugging tools effectively
- Provide step-by-step debugging guidance
```

### Frontmatter Fields

- `description` (optional): Brief description of what the agent does (shown in `list_agents`)
- `model` (optional): Default model to use (e.g., "anthropic:claude-3.5-sonnet", "ollama:llama3.2")

## MCP Configuration (mcp.json)

The optional `mcp.json` file configures which MCP servers the agent has access to:

```json
{
  "servers": {
    "coding": {
      "type": "in-memory"
    }
  }
}
```

The agent process will load this configuration from its working directory, giving it access to the specified tools.

## Available Tools

### list_agents

Lists all available sub-agents with their names and descriptions.

```json
{
  "name": "list_agents",
  "arguments": {}
}
```

Returns:
```json
{
  "agents": [
    {
      "name": "debugger",
      "description": "Debug and fix code issues"
    },
    {
      "name": "code-reviewer",
      "description": "Review code for best practices"
    }
  ]
}
```

### spawn_agent

Spawns a sub-agent to perform a specific task.

```json
{
  "name": "spawn_agent",
  "arguments": {
    "agent_name": "debugger",
    "prompt": "Find and fix the null pointer exception in main.rs",
    "model": "anthropic:claude-3.5-sonnet"  // optional override
  }
}
```

Returns:
```json
{
  "output": "Agent's full output...",
  "success": true
}
```

Parameters:
- `agent_name`: Name of the agent (subdirectory name in `sub-agents/`)
- `prompt`: Task for the agent to perform
- `model` (optional): Override the model specified in AGENT.md

## Example Use Cases

### Debugging Agent
```markdown
---
description: Debug and fix code issues
model: anthropic:claude-3.5-sonnet
---

You are a debugging expert specializing in Rust, Python, and JavaScript.
Focus on finding root causes and providing clear, actionable fixes.
```

### Code Review Agent
```markdown
---
description: Review code for best practices and improvements
model: anthropic:claude-3.5-sonnet
---

You are a senior software engineer conducting code reviews.
Focus on code quality, performance, security, and maintainability.
Provide constructive feedback with specific examples.
```

### Documentation Agent
```markdown
---
description: Generate and improve documentation
model: anthropic:claude-3.5-sonnet
---

You are a technical writer specializing in clear, comprehensive documentation.
Create well-structured docs with examples and best practices.
```

## How It Works

When `spawn_agent` is called:

1. The plugin server loads the agent's `AGENTS.md` file
2. The agent's system prompt (content) is extracted
3. The model is determined (from parameter or frontmatter)
4. The model provider is parsed (e.g., "anthropic:claude-3.5-sonnet")
5. The agent's `mcp.json` is loaded (if present) for tool configuration
6. An MCP manager is spawned with the agent's tools
7. An agent is built with the system prompt and tools
8. The agent runs in-process as a tokio task
9. Agent messages are collected and streamed back
10. Final output and success status are returned

This in-process design provides:
- **No external dependencies** - Uses the aether library directly
- **Better integration** - Proper MCP progress notification support
- **Isolated agent contexts** - Each agent has its own MCP configuration
- **Specialized system prompts** - Custom prompts per agent type
- **Per-agent MCP tool access** - Configure tools via mcp.json
- **Efficient execution** - In-process tokio tasks vs subprocess overhead
- **Parallel agent execution** - Multiple agents can run concurrently
- **Modular agent development** - Easy to add/modify agents

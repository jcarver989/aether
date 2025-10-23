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

Spawns a sub-agent to run as a background task (like spawning a process).

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

Returns immediately with a task ID:
```json
{
  "task_id": "agent-550e8400-e29b-41d4-a716-446655440000"
}
```

Parameters:
- `agent_name`: Name of the agent (subdirectory name in `sub-agents/`)
- `prompt`: Task for the agent to perform
- `model` (optional): Override the model specified in AGENTS.md

### get_agent_output

Gets output from a running or completed agent task.

```json
{
  "name": "get_agent_output",
  "arguments": {
    "task_id": "agent-550e8400-e29b-41d4-a716-446655440000"
  }
}
```

Returns:
```json
{
  "output": "Agent's output so far...",
  "running": true,
  "success": null  // Set when running=false
}
```

Call this repeatedly to stream agent messages. When `running` becomes `false`, the agent has completed and `success` indicates whether it succeeded.

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

### Background Job Architecture

Sub-agents use a **job-based API** similar to background processes with PIDs:

1. **spawn_agent** - Starts an agent and returns a task_id (like a PID)
2. **get_agent_output** - Streams agent messages using the task_id

### When `spawn_agent` is called:

1. The plugin server loads the agent's `AGENTS.md` file
2. The agent's system prompt (content) is extracted
3. The model is determined (from parameter or frontmatter)
4. The model provider is parsed (e.g., "anthropic:claude-3.5-sonnet")
5. The agent's `mcp.json` is loaded (if present) for tool configuration
6. An MCP manager is spawned with the agent's tools
7. An agent is built with the system prompt and tools
8. **A tokio task is spawned** to run the agent in the background
9. The user's prompt is sent to the agent
10. Agent messages are forwarded to a channel
11. **The task_id is returned immediately** (non-blocking)
12. The agent task is stored in a HashMap for later retrieval

### When `get_agent_output` is called:

1. Look up the agent task by task_id in the HashMap
2. Read available messages from the agent's channel (non-blocking)
3. Format and return the messages
4. Check if the agent is still running
5. If complete, remove from HashMap and return final status

### Execution Model

```rust
// spawn_agent: Returns immediately with task_id
let task_id = "agent-uuid...";
agent_tasks.insert(task_id, SpawnedAgent {
    task_handle: tokio::spawn(async { /* agent runs here */ }),
    message_rx: channel_receiver,
});

// get_agent_output: Stream messages without blocking
while let Ok(message) = agent.message_rx.try_recv() {
    // Forward message to caller
    // Can send as MCP progress notification
}
```

### Benefits

- **Non-blocking** - spawn_agent returns immediately with task_id
- **True concurrency** - Multiple agents run in parallel
- **Streamable output** - get_agent_output can be called repeatedly
- **Like background processes** - Familiar PID-based model
- **No external dependencies** - Uses aether library directly
- **Better integration** - Can send MCP progress notifications
- **Isolated agent contexts** - Each agent has its own MCP configuration
- **Specialized system prompts** - Custom prompts per agent type
- **Per-agent MCP tool access** - Configure tools via mcp.json
- **Efficient execution** - In-process tokio tasks
- **Modular agent development** - Easy to add/modify agents

### Usage Pattern

```rust
// Main agent spawns a sub-agent
let result = call_tool("spawn_agent", {
    "agent_name": "debugger",
    "prompt": "Fix the bug in main.rs"
});
let task_id = result.task_id;

// Stream agent output
loop {
    let output = call_tool("get_agent_output", { "task_id": task_id });

    // Display output...

    if !output.running {
        // Agent is done
        println!("Success: {}", output.success);
        break;
    }

    // Wait before polling again
    sleep(1000);
}

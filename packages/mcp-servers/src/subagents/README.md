# SubAgentsMcp

Spawn and orchestrate sub-agents. Sub-agents run in parallel and have access to all parent MCP servers (coding, skills, tasks).

**Flag:** `--dir <path>` (base directory containing `sub-agents/` subdirectory)

## Directory Structure

```
~/.aether/
└── sub-agents/
    ├── explore/
    │   └── AGENTS.md
    ├── rust-engineer/
    │   └── AGENTS.md
    └── ...
```

## Tools

| Tool | Description |
|------|-------------|
| `list_subagents` | List all available sub-agents with their names and descriptions. |
| `spawn_subagent` | Spawn one or more sub-agents with specific prompts. All agents run in parallel. |

## Writing a Sub-Agent

Create a directory under `sub-agents/` with an `AGENTS.md` file:

```markdown
---
description: Explores codebases to answer questions about architecture and patterns
model: anthropic:claude-sonnet-4-20250514
---

# Explorer Agent

You are a codebase exploration specialist. Your job is to...
```

| Frontmatter Field | Description |
|-------------------|-------------|
| `description` | Shown when listing available sub-agents. |
| `model` | LLM model to use for this agent (provider:model format). |

The directory name becomes the agent name: `sub-agents/explore/` -> agent name `explore`.

## Spawning Agents

`spawn_subagent` accepts a list of tasks, each with an `agent_name` and `prompt`. All tasks execute in parallel:

```json
{
  "tasks": [
    { "agent_name": "explore", "prompt": "Find all API endpoints in this codebase" },
    { "agent_name": "explore", "prompt": "Document the database schema" }
  ]
}
```

## Structured Output

Sub-agents are encouraged to return structured output with:

- **summary** -- what was accomplished
- **artifacts** -- files read or modified
- **decisions** -- key decisions made
- **next_steps** -- suggested follow-up actions

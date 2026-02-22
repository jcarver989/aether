# mcp-servers

Pre-built [MCP](https://modelcontextprotocol.io/) tool servers for Aether agents. Each server runs in-process and is gated behind a feature flag so you only compile what you need.

| Feature | Server | What it provides |
|---------|--------|-----------------|
| `coding` | [`CodingMcp`](src/coding/README.md) | File read/write/edit, bash, grep, find, LSP integration, web fetch/search |
| `skills` | [`SkillsMcp`](src/skills/README.md) | Slash commands and reusable skill prompts |
| `tasks` | [`TasksMcp`](src/tasks/README.md) | Hierarchical task management with dependencies |
| `subagents` | [`SubAgentsMcp`](src/subagents/README.md) | Spawn and orchestrate sub-agents |

## Using with Aether (mcp.json)

These servers use Aether's `in-memory` transport type -- they run inside your agent process, not as separate subprocesses. Wire them up in `mcp.json`:

```json
{
  "servers": {
    "coding": {
      "type": "in-memory"
    },
    "skills": {
      "type": "in-memory",
      "args": ["--dir", "$HOME/.aether"]
    },
    "tasks": {
      "type": "in-memory"
    },
    "subagents": {
      "type": "in-memory",
      "args": ["--dir", "$HOME/.aether"]
    }
  }
}
```

Each server key must match a factory registered with `McpBuilder::register_in_memory_server()`. The `args` array is parsed as CLI flags by each server:

| Server | Flag | Default | Description |
|--------|------|---------|-------------|
| `coding` | `--root-dir <path>` | cwd | Workspace root for LSP and file operations |
| `skills` | `--dir <path>` | none | Base directory containing `commands/` and `skills/` subdirectories |
| `tasks` | `--dir <path>` | `.` | Base directory for task storage (creates `.aether-tasks/` inside) |
| `subagents` | `--dir <path>` | none | Base directory containing `sub-agents/` subdirectory |

To register factories and load the config:

```rust
use aether::mcp::mcp;
use futures::FutureExt;
use mcp_servers::{CodingMcp, SkillsMcp, SubAgentsMcp, TasksMcp};

let builder = mcp()
    .register_in_memory_server("coding", Box::new(|_args| {
        async move { CodingMcp::new().into_dyn() }.boxed()
    }))
    .register_in_memory_server("skills", Box::new(|args| {
        async move { SkillsMcp::from_args(args).unwrap().into_dyn() }.boxed()
    }))
    .register_in_memory_server("tasks", Box::new(|args| {
        async move { TasksMcp::from_args(args).unwrap().into_dyn() }.boxed()
    }))
    .register_in_memory_server("subagents", Box::new(|args| {
        async move { SubAgentsMcp::from_args(args).unwrap().into_dyn() }.boxed()
    }));

// Load mcp.json -- matches server keys to registered factories
let builder = builder.from_json_file("mcp.json").await?;
```

## Programmatic Usage

Add to your `Cargo.toml`:

```toml
# Everything
mcp-servers = { path = "../mcp-servers" }

# Just coding tools
mcp-servers = { path = "../mcp-servers", default-features = false, features = ["coding"] }
```

Create and start servers directly:

```rust
use mcp_servers::{CodingMcp, SkillsMcp, TasksMcp};
use rmcp::ServiceExt;

// Create a coding server
let server = CodingMcp::new()
    .with_root_dir("/my/project".into())
    .into_dyn();

// Or with LSP support
use mcp_servers::{DefaultCodingTools, LspCodingTools};

let lsp_tools = LspCodingTools::new(DefaultCodingTools::new(), "/my/project".into());
let server = CodingMcp::with_tools(lsp_tools).into_dyn();
```

## Server Documentation

- [`CodingMcp`](src/coding/README.md)
- [`SkillsMcp`](src/skills/README.md)
- [`TasksMcp`](src/tasks/README.md)
- [`SubAgentsMcp`](src/subagents/README.md)

---

## Feature Flags

- **`default`** -- all four servers
- **`coding`** -- file ops, bash, LSP, web tools
- **`skills`** -- slash commands and prompts
- **`tasks`** -- task tracking (no dependency on `coding`)
- **`subagents`** -- sub-agent spawning (implies `coding`, `skills`, `tasks`)
- **`all`** -- same as default, explicit alias

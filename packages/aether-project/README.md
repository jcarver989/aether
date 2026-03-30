# aether-project

Project-local configuration and agent catalog resolution for the Aether AI agent framework. Reads `.aether/settings.json` to discover agents, prompts, and MCP server configurations.

## Key Types

- **`AgentCatalog`** -- Resolved catalog of project agents with their prompts, models, and tool filters
- **`PromptCatalog`** -- Collection of project prompt files
- **`SettingsError`** -- Configuration validation errors

## Usage

```rust,no_run
use aether_project::load_agent_catalog;
use std::path::Path;

let catalog = load_agent_catalog(Path::new(".")).unwrap();
println!("Project root: {:?}", catalog.project_root());

for agent in catalog.all() {
    println!("Agent: {} (model: {})", agent.name, agent.model);
}
```

## License

MIT

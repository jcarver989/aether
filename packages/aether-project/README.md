# aether-project

Project-local configuration and agent catalog resolution for the Aether AI agent framework. Reads `.aether/settings.json` to discover agents, prompts, and MCP server configurations.

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

- [Key Types](#key-types)
- [Usage](#usage)
- [License](#license)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

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

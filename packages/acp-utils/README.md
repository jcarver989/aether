<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->
**Table of Contents**  *generated with [DocToc](https://github.com/thlorenz/doctoc)*

- [aether-acp-utils](#aether-acp-utils)
  - [Key Types](#key-types)
  - [Feature Flags](#feature-flags)
  - [License](#license)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

# aether-acp-utils

Utilities for the [Agent Client Protocol](https://agentclientprotocol.com/) (ACP), handling notifications, elicitation, and protocol extensions between agents and their host UIs.

## Key Types

- **`ElicitationParams` / `ElicitationResponse`** -- Schema-driven user prompts and responses
- **`ContextUsageParams`** -- Token usage tracking notifications
- **`McpNotification` / `McpRequest`** -- MCP message tunneling over ACP

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `client` | ACP client (for UIs connecting to agents) | yes |
| `server` | ACP server (for agents accepting connections) | yes |

## License

MIT

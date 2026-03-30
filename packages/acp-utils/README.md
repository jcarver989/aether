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

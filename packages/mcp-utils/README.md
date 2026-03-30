# aether-mcp-utils

Utilities for the [Model Context Protocol](https://modelcontextprotocol.io/) (MCP), providing transport, status tracking, and client management for MCP servers.

## Key Types

- **`InMemoryTransport`** -- In-process MCP transport for running servers without subprocesses
- **`McpServerStatus`** -- Tracks server connection state (`Connected`, `Failed`, `NeedsOAuth`)
- **`ToolDisplayMeta` / `ToolResultMeta`** -- Metadata for rendering tool calls and results in UIs

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `client` | MCP client with OAuth, server management, and tool proxying | yes |

## License

MIT

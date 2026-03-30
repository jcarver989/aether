# aether-lspd

An LSP daemon that manages language server processes and shares them across multiple Aether agents. Communicates over Unix domain sockets.

## Key Types

- **`LspDaemon`** -- Main daemon runtime. Listens on a Unix socket and manages language server lifecycles.
- **`LspClient`** -- Client for connecting to a running daemon. Supports go-to-definition, references, hover, diagnostics, rename, and more.
- **`DaemonRequest` / `DaemonResponse`** -- Protocol messages between client and daemon.
- **`LanguageId`** -- Supported language identifiers with associated LSP server configurations.

## Feature Flags

| Feature | Description |
|---------|-------------|
| `testing` | Test utilities for integration tests |

## License

MIT

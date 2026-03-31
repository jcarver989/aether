Standalone MCP server exposing LSP-based code intelligence tools.

Provides language-aware symbol lookup, document structure, diagnostics, workspace search, and rename -- without bundling file I/O or shell tools. Use this when you want LSP capabilities as a separate server from [`CodingMcp`](crate::CodingMcp).

# Construction

```rust,ignore
use mcp_servers::lsp::LspMcp;

let server = LspMcp::new("/my/project".into());
```

# Tools provided

- **`lsp_symbol`** -- Go to definition, find references, find implementations, call hierarchy.
- **`lsp_document`** -- List all symbols in a file (functions, types, fields, etc.).
- **`lsp_check_errors`** -- Get diagnostics for a single file or the entire workspace.
- **`lsp_workspace_search`** -- Search for symbols across the workspace by name.
- **`lsp_rename`** -- Rename a symbol across all files in the project.

# Relationship to `CodingMcp`

[`CodingMcp`](crate::CodingMcp) also exposes LSP tools (all except `lsp_workspace_search`) alongside its file I/O and shell tools when constructed with [`with_lsp`](crate::CodingMcp::with_lsp). `LspMcp` is the standalone alternative for setups that want LSP as a separate server.

# See also

- [`LspRegistry`](crate::lsp::LspRegistry) -- Manages LSP daemon connections (shared between `LspMcp` and `CodingMcp`).
- [`LspError`](crate::lsp::error::LspError) -- Error types for LSP operations.

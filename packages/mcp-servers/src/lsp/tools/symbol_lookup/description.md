**YOUR FIRST CHOICE for code navigation.** Faster and more precise than grep/find.

One `lsp_symbol` call replaces: grep → read file → grep again → read another file

**When to use:**
- "Where is X defined?" → `operation: "definition"`
- "Where is X used?" → `operation: "references"`
- "What type is X?" / "Show docs for X" → `operation: "hover"`
- "What implements this trait/interface?" → `operation: "implementation"`
- "What calls X?" → `operation: "incoming_calls"`
- "What does X call?" → `operation: "outgoing_calls"`

**Usage:**
1. Provide `file_path` and `symbol` (exact name as it appears)
2. Optionally provide `line` (1-indexed) to skip automatic resolution (faster)
3. If `line` is omitted, it is resolved automatically (first via document symbols, then by text search)

## Cross-Crate Navigation

**This is the key to fast dependency exploration:**

Instead of manually navigating to `/Users/josh/.cargo/registry/src/.../rmcp/`,
you can jump directly:

1. Start from an import in your code
2. Use `lsp_symbol(operation: "definition")` on the type
3. You're now in the dependency's source

**Example workflow:**
```text
# In your code:
use rmcp::{ServerHandler, ...};

# Jump to ServerHandler:
lsp_symbol(file_path: "/path/to/file.rs", symbol: "ServerHandler", operation: "definition")

# Find all callers of a function:
lsp_symbol(file_path: "/path/to/file.rs", symbol: "process_request", operation: "incoming_calls")
```

**Example - Find definition:**
```json
{
  "operation": "definition",
  "file_path": "/path/to/file.rs",
  "symbol": "HashMap"
}
```

**Example - Find all usages:**
```json
{
  "operation": "references",
  "file_path": "/path/to/file.rs",
  "symbol": "process_request"
}
```

**Example - Find all callers (one-step call hierarchy):**
```json
{
  "operation": "incoming_calls",
  "file_path": "/path/to/file.rs",
  "symbol": "process_request"
}
```

Note: `include_declaration` parameter only applies to `references` (default: true).

## Controlling output size

**`limit`** — cap the number of results returned (locations or call-hierarchy items).
`total_count` always reflects the full count; `truncated: true` appears when results were capped.

Use `limit: 20` for `incoming_calls`/`outgoing_calls` on large functions to avoid oversized responses:
```json
{
  "operation": "outgoing_calls",
  "file_path": "/path/to/file.rs",
  "symbol": "process_request",
  "limit": 20
}
```

**`context_lines`** — include N lines of source code around each location (definition, implementation, references only).
Eliminates the need to call `read_file` after getting results:
```json
{
  "operation": "references",
  "file_path": "/path/to/file.rs",
  "symbol": "process_request",
  "context_lines": 2,
  "limit": 10
}
```

## Notes on `outgoing_calls`

Results include **all** calls the function makes, including standard library and
dependency calls (`map_err`, `collect`, `to_string`, `Ok`, etc.). This is normal
LSP behavior. To manage noise:

- Use `limit` to cap the result count (e.g., `limit: 20`)
- Filter results by `file_path` — project-local calls are usually what you want
- Stdlib/dep calls can be useful for understanding data flow, but skip them when
  looking for architectural relationships

## Finding symbols across the workspace

If you don't know which file a symbol lives in, use `lsp_workspace_search` instead.
It performs a workspace-wide query without requiring a file path.

## Anti-patterns (don't do this)

- `grep "fn process_request"` then `read_file` to find it → use `lsp_symbol(operation: "definition")`
- `grep "HashMap"` (matches comments, strings, imports) → use `lsp_symbol(operation: "references")` for only real usages
- Navigating to `~/.cargo/registry/...` manually → use `lsp_symbol` on import to jump directly into dependency source

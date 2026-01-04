**YOUR FIRST CHOICE for code navigation.** Faster and more precise than grep/find.

One `lsp_symbol` call replaces: grep → read file → grep again → read another file

**When to use:**
- "Where is X defined?" → `operation: "definition"`
- "Where is X used?" → `operation: "references"`
- "What type is X?" / "Show docs for X" → `operation: "hover"`
- "What implements this trait/interface?" → `operation: "implementation"`
- "I need call hierarchy for X" → `operation: "prepare_call_hierarchy"` (then use `lsp_call_hierarchy`)

**Usage:**
1. Provide `file_path`, `symbol` (exact name as it appears), and `line` (1-indexed)
2. The file should be read first (establishes LSP context)

## Cross-Crate Navigation

**This is the key to fast dependency exploration:**

Instead of manually navigating to `/Users/josh/.cargo/registry/src/.../rmcp/`,
you can jump directly:

1. Start from an import in your code
2. Use `lsp_symbol(operation: "definition")` on the type
3. You're now in the dependency's source

**Example workflow:**
```
# In your code:
use rmcp::{ServerHandler, ...};

# Jump to ServerHandler:
lsp_symbol(symbol: "ServerHandler", operation: "definition", line: 3)

# Then from there, jump to model:
lsp_symbol(symbol: "model", operation: "definition", line: 3)

# Then find CallToolResult:
grep("struct CallToolResult", path: "model.rs")
```

**Example - Find definition:**
```json
{
  "operation": "definition",
  "file_path": "/path/to/file.rs",
  "symbol": "HashMap",
  "line": "15"
}
```

**Example - Find all usages:**
```json
{
  "operation": "references",
  "file_path": "/path/to/file.rs",
  "symbol": "process_request",
  "line": "42"
}
```

Note: `include_declaration` parameter only applies to `references` (default: true).

## Anti-patterns (don't do this)

- `grep "fn process_request"` then `read_file` to find it → use `lsp_symbol(operation: "definition")`
- `grep "HashMap"` (matches comments, strings, imports) → use `lsp_symbol(operation: "references")` for only real usages
- Navigating to `~/.cargo/registry/...` manually → use `lsp_symbol` on import to jump directly into dependency source

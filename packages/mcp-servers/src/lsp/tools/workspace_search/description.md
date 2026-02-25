Search for symbols across the **entire workspace** by name.

Unlike `lsp_symbol` (which operates on a known file + symbol position), this tool
performs a fuzzy/substring query across all files the LSP knows about — ideal when
you don't know which file a symbol lives in.

**When to use:**
- "Where is `AppState` defined?" (you don't know the file)
- "Find all structs matching `Repository`"
- "Which module declares `process_request`?"

**When `lsp_symbol` is better:**
- You already know the file and want definition/references/hover/calls

**Examples:**

Find a struct by name:
```json
{ "query": "AppState" }
```

Limit results and include context:
```json
{ "query": "Repository", "limit": 10, "context_lines": 3 }
```

**Notes:**
- Query matching is LSP-server dependent (typically substring/fuzzy)
- Results are deduplicated across language servers
- Use `limit` to cap large result sets

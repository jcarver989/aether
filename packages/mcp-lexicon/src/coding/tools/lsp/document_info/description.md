**Use BEFORE reading large files.** Get the map, then read only what matters.

**When to use:**
- "What's in this file?" - Get a map of file structure
- "Where is function X in this file?" - Find it without reading the whole file
- After reading a file, to understand its organization

## When to Use This

**Before reading an entire file:**
- Get the file's structure with `lsp_document`
- Identify which functions/symbols you actually care about
- Read only those sections with `read_file(offset, limit)`

**Example:**
```
# Don't read 800 lines of model.rs blindly
# Instead:
lsp_document(file_path: "model.rs", operation: "symbols")
# → Shows all types, line numbers, kinds

# Then read only what you need:
read_file(offset: 1530, limit: 50, file_path: "model.rs")
```

**Usage:**
```json
{
  "operation": "symbols",
  "file_path": "/path/to/file.rs"
}
```

**Returns:** Hierarchical list of symbols with names, kinds (function, class, struct, etc.), and line locations.

Note: Read file first to establish LSP context.

## The 800-line trap

- `read_file("model.rs")` → 800 lines of context pollution
- `lsp_document("model.rs")` → see all symbols → `read_file(offset: 150, limit: 30)`

This keeps your context clean and focused.

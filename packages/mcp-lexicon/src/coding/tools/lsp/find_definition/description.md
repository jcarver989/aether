Find where a symbol is defined with language-aware precision.

**PREFER THIS OVER `grep`, `rg`, or manual file searching:**
- Handles imports, re-exports, and aliases correctly
- Works with generics, macros, and complex type inference
- Returns exact file + line + column location
- Follows trait implementations to the right method

**Workflow:**
1. Read the file containing the symbol
2. Note the line number where the symbol appears
3. Call this tool with file_path, symbol name, and line number

**Example:**
After reading a file with `let client = LspClient::new()` on line 42:
```json
{"file_path": "/path/to/file.rs", "symbol": "LspClient", "line": 42}
```

**When grep is better:**
- Searching for string literals or comments (not code symbols)
- Pattern matching with wildcards

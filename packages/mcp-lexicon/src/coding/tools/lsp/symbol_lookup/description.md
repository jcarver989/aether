Look up information about a symbol using LSP (Language Server Protocol).

This tool provides multiple operations for symbol navigation and information:

**Operations:**
- `definition`: Navigate to where a symbol is defined
- `implementation`: Find implementations of an interface/trait method
- `references`: Find all usages of a symbol across the codebase
- `hover`: Get type information and documentation for a symbol
- `prepare_call_hierarchy`: Get call hierarchy items for use with lsp_call_hierarchy

**Usage:**
1. Read the file containing the symbol first (required for LSP context)
2. Provide the exact symbol name as it appears in the code
3. Provide the 1-indexed line number where the symbol appears

**Example - Find definition:**
```json
{
  "operation": "definition",
  "file_path": "/path/to/file.rs",
  "symbol": "HashMap",
  "line": "15"
}
```

**Example - Find all references:**
```json
{
  "operation": "references",
  "file_path": "/path/to/file.rs",
  "symbol": "my_function",
  "line": "42",
  "include_declaration": false
}
```

**Note:** The `include_declaration` parameter only applies to the `references` operation.

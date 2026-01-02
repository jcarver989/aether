Get information about a document using LSP (Language Server Protocol).

**Operations:**
- `symbols`: Get all symbols (functions, classes, variables, etc.) in the document

**Usage:**
1. Read the file first to ensure it's open in the LSP
2. Use this tool to get structural information about the document

**Example - Get all symbols:**
```json
{
  "operation": "symbols",
  "file_path": "/path/to/file.rs"
}
```

**Returns:**
- `symbols`: Hierarchical list of symbols with their names, kinds, and locations
- `total_count`: Number of top-level symbols

Symbols are returned with their full hierarchical structure when supported by the language server.

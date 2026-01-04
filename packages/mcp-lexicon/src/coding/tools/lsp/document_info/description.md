Get a structural overview of a file - all functions, classes, structs, and their locations.

**When to use:**
- "What's in this file?" - Get a map of the file structure
- "Where is function X in this file?" - Find it without reading the whole file
- After reading a file, to understand its organization

**Usage:**
```json
{
  "operation": "symbols",
  "file_path": "/path/to/file.rs"
}
```

**Returns:** Hierarchical list of symbols with names, kinds (function, class, struct, etc.), and line locations.

Note: Read the file first to establish LSP context.

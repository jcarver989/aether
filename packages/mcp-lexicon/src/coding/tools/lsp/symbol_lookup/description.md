Navigate code semantically using LSP. Unlike grep, LSP understands code structure.

**When to use:**
- "Where is X defined?" → `operation: "definition"`
- "Where is X used?" → `operation: "references"`
- "What type is X?" / "Show docs for X" → `operation: "hover"`
- "What implements this trait/interface?" → `operation: "implementation"`
- "I need call hierarchy for X" → `operation: "prepare_call_hierarchy"` (then use `lsp_call_hierarchy`)

**Usage:**
1. Provide `file_path`, `symbol` (exact name as it appears), and `line` (1-indexed)
2. The file should be read first (establishes LSP context)

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

Navigate the call hierarchy of a function or method using LSP.

This tool works with call hierarchy items obtained from `lsp_symbol` with `operation: "prepare_call_hierarchy"`.

**Operations:**
- `incoming`: Find all functions/methods that call this item (who calls me?)
- `outgoing`: Find all functions/methods that this item calls (who do I call?)

**Workflow:**
1. First, use `lsp_symbol` with `operation: "prepare_call_hierarchy"` to get a call hierarchy item
2. Pass the item to this tool to explore incoming or outgoing calls

**Example - Find callers:**
```json
{
  "direction": "incoming",
  "item": { /* CallHierarchyItemResult from lsp_symbol */ }
}
```

**Example - Find callees:**
```json
{
  "direction": "outgoing",
  "item": { /* CallHierarchyItemResult from lsp_symbol */ }
}
```

**Returns:**
- `calls`: List of call sites with the calling/called item and specific call locations
- `total_count`: Total number of call sites found

Explore call relationships: who calls a function, and what does it call.

**When to use:**
- "What calls this function?" → `direction: "incoming"`
- "What does this function call?" → `direction: "outgoing"`
- Tracing code flow, understanding dependencies, impact analysis

**Workflow:**
1. Call `lsp_symbol` with `operation: "prepare_call_hierarchy"` to get an `item`
2. Pass that `item` to this tool with a `direction`

**Example - Find all callers:**
```json
{
  "direction": "incoming",
  "item": { /* CallHierarchyItemResult from step 1 */ }
}
```

**Returns:** List of call sites with the calling/called function and exact locations.

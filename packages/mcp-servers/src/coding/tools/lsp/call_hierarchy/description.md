Explore call relationships using a previously prepared `CallHierarchyItem`.

**Prefer `lsp_symbol`** with `operation: "incoming_calls"` or `"outgoing_calls"` instead — it handles preparation automatically in one step.

This tool is only for advanced use cases where you already have a `CallHierarchyItem` from a prior operation.

**When to use:**
- "What calls this function?" → `direction: "incoming"`
- "What does this function call?" → `direction: "outgoing"`

**Example:**
```json
{
  "direction": "incoming",
  "item": { /* CallHierarchyItemResult from a prior operation */ }
}
```

**Returns:** List of call sites with the calling/called function and exact locations.

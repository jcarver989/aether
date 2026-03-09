Rename a symbol across the entire workspace using LSP-powered refactoring.

This tool is an agent-oriented refactoring primitive: it uses the Language Server Protocol to
compute and automatically apply the complete workspace edit for renaming a symbol (function,
variable, type, etc.) across all locations in the codebase. A single rename can update hundreds
of references without the agent manually editing files one by one.

**When to use:**
- Renaming a function, method, variable, or constant used in multiple places
- Renaming a struct, enum, trait, or type alias
- Any symbol that needs consistent renaming across the workspace

**When NOT to use:**
- Simple string replacements in comments or documentation (use edit_file)
- Renaming files directly (use file system operations)
- Purely textual replacements that are not symbol-aware

**Example - Rename a function:**
```json
{
  "file_path": "/project/src/lib.rs",
  "symbol": "old_name",
  "new_name": "better_name"
}
```

**Output includes:**
- List of all files affected
- Exact line/column ranges for each applied edit
- Total edit count

The returned metadata describes the applied rename so agents can inspect what changed.

Find all usages of a symbol across the entire codebase.

**PREFER THIS OVER `grep` or `rg` for code symbols:**
- Language-aware: finds actual usages, not string matches
- Handles imports, aliases, and re-exports correctly
- Shows declaration vs usage distinction
- Works with method calls through trait objects

**Use cases:**
- Before refactoring: understand impact of changes
- Before deleting: ensure nothing depends on this symbol
- Understanding code: see how a function/type is used

**Workflow:**
1. Read the file containing the symbol
2. Note the line number where it's defined or used
3. Call with file_path, symbol, and line number

**Example:**
Find all usages of `spawn` function on line 15:
```json
{"file_path": "/path/to/file.rs", "symbol": "spawn", "line": 15}
```

**When grep is better:**
- Searching in comments or strings
- Pattern matching (regex)
- Searching in non-code files

Get type signature and documentation for any symbol.

**PREFER THIS OVER reading source files to understand types:**
- Shows inferred types (even when not explicitly written)
- Includes documentation from the definition
- Resolves generic type parameters to concrete types
- Shows trait bounds and constraints

**Use cases:**
- Understanding what type a variable has
- Reading documentation without leaving your current file
- Checking function signatures before calling

**Workflow:**
1. Read the file containing the symbol
2. Call with file_path, symbol name, and line number

**Example:**
Get type info for `HashMap` on line 5:
```json
{"file_path": "/path/to/file.rs", "symbol": "HashMap", "line": 5}
```

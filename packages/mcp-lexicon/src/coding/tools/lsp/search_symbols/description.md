Search for functions, types, and other symbols across the workspace.

**PREFER THIS OVER `grep` or `find` when looking for code definitions:**
- Fuzzy matching: "LspCli" finds "LspClient"
- Knows symbol kinds: functions, structs, traits, etc.
- Returns exact locations (file + line + column)
- Searches indexed symbols (faster than file scanning)

**Use cases:**
- Finding where a type/function is defined when you don't know the file
- Exploring a codebase to understand its structure
- Jumping to a definition by name

**Example:**
```json
{"query": "LspClient"}
```

Returns all symbols matching "LspClient" with their locations.

**When grep/find is better:**
- Searching for string literals in code
- Searching in non-code files
- Complex regex patterns

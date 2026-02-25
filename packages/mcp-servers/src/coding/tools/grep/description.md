Text/pattern search. **For code structure (definitions, usages, types), use `lsp_symbol` instead—it's faster and understands code.**

Best for: string literals, log messages, TODOs, comments, or regex patterns in non-code files.

Usage:
- Supports full regex syntax (e.g., "log.*Error", "function\s+\w+")
- Filter files with glob parameter (e.g., "*.js", "**/*.tsx") or type parameter (e.g., "js", "py", "rust")
- Output modes: "content" shows matching lines, "`files_with_matches`" shows file paths (default), "count" shows match counts
- Pattern syntax: Uses ripgrep - literal braces need escaping (use `interface\{\}` to find `interface{}` in Go code)
- Multiline matching: For cross-line patterns like `struct \{[\s\S]*?field`, use `multiline: true`
- Call multiple grep in parallel when speculative searches are useful

## When NOT to use grep

- "Where is function X defined?" → `lsp_symbol(operation: "definition")`
- "What calls function X?" → `lsp_symbol(operation: "incoming_calls")`
- "What type is variable X?" → `lsp_symbol(operation: "hover")`

Grep can't distinguish `foo` the function from `foo` in a comment or string. LSP can.

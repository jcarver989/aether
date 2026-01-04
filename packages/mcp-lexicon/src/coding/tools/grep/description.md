Text/pattern search using ripgrep. Best for string literals, log messages, TODOs, or regex patterns.

**Use `lsp_symbol` instead for:** finding definitions, usages, or type information. LSP understands code structure; grep just matches text.

Usage:
- Supports full regex syntax (e.g., "log.*Error", "function\s+\w+")
- Filter files with glob parameter (e.g., "*.js", "**/*.tsx") or type parameter (e.g., "js", "py", "rust")
- Output modes: "content" shows matching lines, "files_with_matches" shows file paths (default), "count" shows match counts
- Pattern syntax: Uses ripgrep - literal braces need escaping (use `interface\{\}` to find `interface{}` in Go code)
- Multiline matching: For cross-line patterns like `struct \{[\s\S]*?field`, use `multiline: true`
- Call multiple grep in parallel when speculative searches are useful

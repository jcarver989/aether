Finds files by name pattern. **For finding where code symbols are defined, use `lsp_symbol` instead.**

Best for: locating files when you know part of the filename (e.g., "*.test.ts", "config*.json")

Usage:
- Supports glob patterns like "**/*.js" or "src/**/*.ts"
- Returns matching file paths sorted alphabetically
- Use this tool when you need to find files by name patterns
- When doing an open-ended search that may require multiple rounds of globbing and grepping, use the Task tool instead
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful.

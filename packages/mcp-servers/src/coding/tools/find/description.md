Finds files by name pattern.

**For code symbols, use `lsp_symbol` instead.** This tool is for locating files by filename.

## Usage

```json
{"pattern": "**/*.rs"}
{"pattern": "*.test.ts", "path": "/path/to/search"}
```

- `pattern` — **required**, glob pattern (e.g., `**/*.js`, `config*.json`)
- `path` — directory to search (default: cwd)

**Returns:** matching file paths, sorted alphabetically.

## Tips

- Run multiple searches in parallel when exploring
- For open-ended searches requiring multiple rounds, consider spawning a sub-agent

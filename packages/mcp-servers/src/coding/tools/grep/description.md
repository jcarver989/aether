Text/pattern search using ripgrep.

**For code structure (definitions, usages, types), use `lsp_symbol` instead.** Grep is for string literals, TODOs, logs, and non-code files.

## Usage

```json
{"pattern": "TODO|FIXME"}
{"pattern": "error.*failed", "outputMode": "content", "-n": true}
{"pattern": "impl Handler", "type": "rust"}
{"pattern": "*.test.ts", "glob": "*.ts"}
```

- `pattern` — **required**, regex pattern
- `outputMode` — `files_with_matches` (default), `content`, or `count`
- `type` — file type filter (`js`, `py`, `rust`, etc.)
- `glob` — glob pattern filter (e.g., `*.ts`)
- `-n` — show line numbers (for content mode)
- `-C` — context lines around matches
- `multiline` — for cross-line patterns

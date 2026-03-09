Renames a symbol across the entire workspace using LSP-powered refactoring.

A single rename updates all references — no manual file-by-file editing needed.

## Usage

```json
{"file_path": "/project/src/lib.rs", "symbol": "old_name", "new_name": "better_name"}
```

- `file_path` — **required**, file containing the symbol
- `symbol` — **required**, current symbol name
- `new_name` — **required**, new symbol name
- `line` — optional, 1-indexed line (skips auto-resolution)

**Returns:** files affected, line/column ranges, total edit count.

## When to Use

- Renaming functions, methods, variables, constants
- Renaming structs, enums, traits, type aliases
- Any symbol needing consistent workspace-wide rename

## When NOT to Use

- String replacements in comments/docs → `edit_file`
- Renaming files → file system operations

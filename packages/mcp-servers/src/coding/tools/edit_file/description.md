Performs exact string replacements in files.

## Usage

```json
{"filePath": "/path/to/file.rs", "oldString": "foo", "newString": "bar"}
{"filePath": "/path/to/file.rs", "oldString": "old_name", "newString": "new_name", "replaceAll": true}
```

- `filePath` — **required**, absolute path
- `oldString` — **required**, exact string to find (must be unique unless `replaceAll`)
- `newString` — **required**, replacement text
- `replaceAll` — replace all occurrences (default: false)

## Tips

- Preserve exact indentation from `read_file` output — match text AFTER the tab character, not the line number prefix
- For renaming symbols across the codebase, use `lsp_rename` instead
- Prefer editing existing files over creating new ones

## Safety

You MUST read a file with `read_file` before editing it. This prevents accidental data loss.

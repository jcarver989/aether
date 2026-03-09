**Use BEFORE reading large files.** Get the map, then read only what matters.

## Workflow

```
lsp_document(file_path) → see all symbols & line numbers → read_file(offset, limit)
```

**Avoid the 800-line trap:** Reading an entire file pollutes context. Get the structure first, then read only what you need.

## Usage

```json
{"file_path": "/path/to/file.rs"}
```

**Returns:** Hierarchical list of symbols with names, kinds (function, class, struct, etc.), and line locations.

Note: Read file first to establish LSP context.

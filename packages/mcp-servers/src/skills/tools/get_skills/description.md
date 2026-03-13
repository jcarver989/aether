Loads files from skill directories. If you omit `path`, it loads the skill's `SKILL.md`. If you provide `path`, it loads that file relative to the skill root.

When loading `SKILL.md`, the server also returns a manifest of additional available files so you can progressively load more context only as needed.

## Usage

### Load a skill root (SKILL.md)

```json
{
  "requests": [
    { "name": "rust" }
  ]
}
```

### Load specific additional files

```json
{
  "requests": [
    { "name": "rust", "path": "traits.md" },
    { "name": "rust", "path": "error-handling.md" }
  ]
}
```

### Mix root and auxiliary loads in one call

```json
{
  "requests": [
    { "name": "rust" },
    { "name": "rust", "path": "tooling.md" }
  ]
}
```

## Parameters

- `requests` — **required**, array of skill requests
  - `name` — **required**, skill directory name
  - `path` — optional, file path relative to skill root (defaults to `SKILL.md`)

## Response

Returns an array of `files` with:

- `name` — skill name
- `path` — file path (normalized to `SKILL.md` if omitted in request)
- `content` — file content (null if error)
- `error` — error message if file could not be loaded
- `availableFiles` — list of auxiliary files in the skill (only for `SKILL.md`)

## Supported File Types

Text files only: `.md`, `.txt`, `.json`, `.yaml`, `.yml`, `.toml`, `.rs`, `.py`, `.sh`, `.bash`, `.js`, `.ts`, `.go`, `.java`, `.c`, `.cpp`, `.h`, `.hpp`, `.css`, `.html`, `.xml`, `.sql`, `.env`, `.ini`, `.cfg`, `.conf`

Binary files return an error.

## Security

The server validates paths to prevent:
- Absolute paths
- Path traversal (`..`)
- Access outside the skill directory
- Directory paths (only files allowed)

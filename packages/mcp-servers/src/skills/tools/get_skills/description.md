Loads files from skill directories. If you omit `path`, it loads the skill's `SKILL.md`. If you provide `path`, it loads that file relative to the skill root.

When loading `SKILL.md`, the server also returns a manifest of additional available files so you can progressively load more context only as needed.

## Usage

```json
{
  "requests": [
    { "name": "rust" },
    { "name": "rust", "path": "traits.md" }
  ]
}
```

## Parameters

- `requests` — **required**, array of skill requests
  - `name` — **required**, skill directory name
  - `path` — optional, file path relative to skill root (defaults to `SKILL.md`)

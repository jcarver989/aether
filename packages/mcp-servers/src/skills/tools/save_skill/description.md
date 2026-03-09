Creates or updates an agent-authored skill.

Use this when you learn something worth remembering: conventions, pitfalls, debugging insights, codebase facts. One concept per skill.

## Usage

```json
{"name": "my-insight", "description": "Short description", "content": "# Title\n\nDetailed markdown content..."}
```

- `name` — **required**, skill directory name
- `description` — **required**, short description for table of contents
- `content` — **required**, full markdown content
- `tags` — optional array of tags (e.g., `["rust", "testing"]`)

## Behavior

- New skills are created with `agent_authored: true`
- Existing agent-authored skills are updated (ratings preserved)
- Human-authored skills cannot be overwritten

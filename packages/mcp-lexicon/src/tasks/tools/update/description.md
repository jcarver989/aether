Update an existing task's fields or mark it as completed.

Use this to modify a task's title, description, status, assignee, dependencies, or to complete a task with a result.

Available status values:
- `pending`: Task is waiting to be started
- `in_progress`: Task is currently being worked on
- `blocked`: Task cannot proceed (waiting for external input)
- `completed`: Task is finished (requires `result` field)

Usage:
- Provide the task `id` and any fields you want to update
- Only specified fields will be changed; others remain as-is
- Dependencies (`deps`) are replaced entirely if provided
- To complete a task, provide a `result` (status will auto-set to `completed`)

## Completing Tasks

When completing a task, provide a `result` with:

| Field | Purpose |
|-------|---------|
| `summary` | 1-3 sentence headline of what was accomplished (required) |
| `handoff` | Context for downstream agents (optional) |

The `handoff` object can contain:
- `decisions`: Key decisions made ("Chose X because Y")
- `facts`: Important discoveries ("Found: error X in file Y")
- `next_steps`: Suggested follow-up actions
- `blockers`: Unresolved issues
- `files_read`: Files examined (git tracks modifications)
- `resources`: External resources accessed ("https://... - description")

When a task is completed, `newly_ready` returns tasks that were waiting on this one.

## Examples

**Start working on a task:**
```json
{
  "id": "at-a1b2c3d4.1",
  "status": "in_progress",
  "assignee": "worker-1"
}
```

**Add dependencies:**
```json
{
  "id": "at-a1b2c3d4.2",
  "deps": ["at-a1b2c3d4.1"]
}
```

**Complete a task (minimal):**
```json
{
  "id": "at-a1b2c3d4.1",
  "result": {
    "summary": "Fixed typo in README"
  }
}
```

**Complete a task (with handoff context):**
```json
{
  "id": "at-a1b2c3d4.1",
  "result": {
    "summary": "Identified 5 API endpoints using deprecated auth",
    "handoff": {
      "decisions": [
        "Defer JWT migration until after v2.0 - breaking change requires SDK updates"
      ],
      "facts": [
        "All endpoints use validate_session() for auth (src/api/*.rs)",
        "Session tokens expire after 1 hour with no refresh"
      ],
      "next_steps": ["Create migration guide", "Add deprecation warnings"],
      "blockers": ["Need product decision on migration timeline"],
      "files_read": ["src/api/auth.rs", "docs/AUTH.md"],
      "resources": ["https://docs.rs/jsonwebtoken - supports RS256/ES256"]
    }
  }
}
```

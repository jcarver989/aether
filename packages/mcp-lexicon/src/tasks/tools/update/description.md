Update an existing task's fields or mark it as completed.

Use this to modify a task's title, description, status, assignee, or dependencies.
To complete a task, provide a `result` object.

## Updating Fields

Provide the task `id` and any fields you want to update. Only specified fields will be changed.

Available status values for the `status` field:
- `pending`: Task is waiting to be started
- `in_progress`: Task is currently being worked on
- `blocked`: Task cannot proceed (waiting for external input)

**Note:** Do not set `status` to `completed` directly. Instead, provide a `result` object.

## Completing Tasks

To complete a task, provide a `result` object. The task status will automatically be set to `completed`.

The `result` object fields:

| Field | Required | Description |
|-------|----------|-------------|
| `summary` | Yes | 1-3 sentence summary of what was accomplished |
| `decisions` | No | Key decisions made ("Chose X because Y") |
| `facts` | No | Important discoveries ("Found: error X in file Y") |
| `next_steps` | No | Suggested follow-up actions |
| `blockers` | No | Unresolved issues |
| `files_read` | No | Files examined (git tracks modifications) |
| `resources` | No | External resources accessed ("https://... - description") |

When a task is completed, the response includes `newly_ready` - tasks that were waiting on this one.

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

**Complete a task (with context):**
```json
{
  "id": "at-a1b2c3d4.1",
  "result": {
    "summary": "Identified 5 API endpoints using deprecated auth",
    "decisions": ["Defer JWT migration until after v2.0 - breaking change requires SDK updates"],
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
```

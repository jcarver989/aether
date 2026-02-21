Update any fields on a task.

## Fields

| Field | Description |
|-------|-------------|
| `id` | Task ID (required) |
| `title` | New title |
| `description` | New description (markdown) |
| `status` | `pending`, `in_progress`, `completed`, or `blocked` |
| `assignee` | Agent/worker assigned |
| `deps` | Task IDs this depends on |
| `summary` | Summary of work done |
| `decisions` | Key decisions made |
| `facts` | Important discoveries |
| `next_steps` | Suggested follow-up |
| `blockers` | Unresolved issues |
| `files_read` | Files examined |
| `resources` | External resources |

## Examples

**Start a task:**
```json
{"id": "at-123.1", "status": "in_progress"}
```

**Complete a task:**
```json
{"id": "at-123.1", "status": "completed", "summary": "Fixed the bug"}
```

**Complete with context:**
```json
{
  "id": "at-123.1",
  "status": "completed",
  "summary": "Found 5 endpoints with deprecated auth",
  "facts": ["All use validate_session()"],
  "next_steps": ["Create migration guide"]
}
```

Updates fields on a task.

## Usage

```json
{"id": "at-123.1", "status": "in_progress"}
{"id": "at-123.1", "status": "completed", "summary": "Fixed the bug"}
{"id": "at-123.1", "status": "completed", "summary": "Found 5 issues", "facts": ["All use deprecated API"], "next_steps": ["Migrate to new API"]}
```

## Fields

| Field | Description |
|-------|-------------|
| `id` | **required**, task ID |
| `status` | `pending`, `in_progress`, `completed`, `blocked` |
| `title` | new title |
| `description` | new description (markdown) |
| `assignee` | agent/worker assigned |
| `deps` | task IDs this depends on |
| `summary` | summary of work done |
| `decisions` | key decisions made |
| `facts` | important discoveries |
| `next_steps` | suggested follow-up |
| `blockers` | unresolved issues |
| `files_read` | files examined |
| `resources` | external resources |

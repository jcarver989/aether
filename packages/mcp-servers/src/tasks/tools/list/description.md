Lists tasks with optional filters.

## Usage

```json
{}
{"assignee": "worker-1"}
{"status": "in_progress"}
{"tree_id": "at-a1b2c3d4"}
{"ready_only": true}
```

## Filters

| Filter | Description |
|--------|-------------|
| `assignee` | filter by agent/worker |
| `status` | `pending`, `in_progress`, `completed`, `blocked` |
| `tree_id` | list all tasks in a specific tree |
| `ready_only` | only tasks ready to start (pending, all deps completed) |

No filters → returns all active tasks.

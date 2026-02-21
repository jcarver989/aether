List tasks with optional filters.

Use this to query tasks by various criteria: assignee, status, tree, or readiness.

Filter options:
- `assignee`: Filter by agent/worker assigned to tasks
- `status`: Filter by status (pending, `in_progress`, completed, blocked)
- `tree_id`: List all tasks in a specific task tree
- `ready_only`: Only return tasks ready to start (pending with all deps completed)

If no filters are provided, returns all active tasks.

Example - Get tasks assigned to a worker:
```json
{
  "assignee": "worker-1"
}
```

Example - Get tasks ready to start:
```json
{
  "ready_only": true
}
```

Example - List all tasks in a tree:
```json
{
  "tree_id": "at-a1b2c3d4"
}
```

Example - Get in-progress tasks:
```json
{
  "status": "in_progress"
}
```

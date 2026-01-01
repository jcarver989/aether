Update an existing task's fields.

Use this to modify a task's title, description, status, assignee, or dependencies. To mark a task as completed, use `task_complete` instead.

Available status values:
- `pending`: Task is waiting to be started
- `in_progress`: Task is currently being worked on
- `blocked`: Task cannot proceed (waiting for external input)

Usage:
- Provide the task `id` and any fields you want to update
- Only specified fields will be changed; others remain as-is
- Dependencies (`deps`) are replaced entirely if provided

Example - Start working on a task:
```json
{
  "id": "at-a1b2c3d4.1",
  "status": "in_progress",
  "assignee": "worker-1"
}
```

Example - Add dependencies:
```json
{
  "id": "at-a1b2c3d4.2",
  "deps": ["at-a1b2c3d4.1"]
}
```

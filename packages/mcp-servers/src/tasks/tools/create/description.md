Create a new task or subtask for tracking work in deep research workflows.

Tasks are organized into trees: a root task can have multiple subtasks, and subtasks can have dependencies on other tasks.

Usage:
- To create a root task tree: provide `title`; `description`, `assignee`, and `deps` are optional
- To create a subtask: provide `parent_id` of an existing task; `description`, `assignee`, and `deps` are optional
- Empty or whitespace-only `parent_id` is treated the same as omitting it
- Use `deps` to specify tasks that must complete before this task can start
- Use `assignee` to assign the task to a specific agent/worker

Task IDs:
- Root tasks: `at-{hash}` (e.g., `at-a1b2c3d4`)
- Subtasks: `at-{hash}.{n}` (e.g., `at-a1b2c3d4.1`)

Example - Create a research tree:
```json
{
  "title": "Research multi-agent systems",
  "description": "Investigate orchestrator-worker patterns",
  "assignee": "orchestrator"
}
```

Example - Add a subtask:
```json
{
  "title": "Analyze paper X",
  "description": "Review methodology and summarize findings",
  "parent_id": "at-a1b2c3d4",
  "assignee": "worker-1"
}
```

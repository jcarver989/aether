Spawns sub-agents in parallel to perform concurrent tasks.

## Usage

```json
{
  "tasks": [
    {"agentName": "codebase-explorer", "prompt": "Find all API endpoints"},
    {"agentName": "rust-code-monkey", "prompt": "Write tests for auth module"}
  ]
}
```

- `tasks` — **required**, array of task objects
  - `agentName` — agent from sub-agents directory
  - `prompt` — task for the agent to perform

All agents execute in parallel. Results returned when ALL complete.

**Returns per agent:** summary, artifacts, decisions, `next_steps`

## Ideal For

- Parallel codebase exploration
- Concurrent file analysis
- Multi-aspect code review

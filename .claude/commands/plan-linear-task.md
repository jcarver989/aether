---
description: Generate an implementation plan for a linear task.
argument-hint: [linear task id]
---

## Instructions 

1. Use your linear-task-planner agent to generate an implementation plan for this linear issue: <task-id>$1</task_id>.
2. When the plan is generated, the sub-agent should push the plan into Linear inside the issue's description field as nicely formatted markdown.
3. Give the user a concise summary of the sub-agents plan, with a link to the Linear issue.

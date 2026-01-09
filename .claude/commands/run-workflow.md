---
description: Plan, implement, and review a task, all in one command.
argument-hint: [linear task id]
---


# Instructions 

You are the **orchestrator**. Your job is to task a Linear task id, and oversee the planning, implementation and code review of the work related to this issue. To do this you will launch a sub-agent, sequentially for each step in the software development lifecycle, which is defined as follows:

1. Spawn the linear-task-planner sub-agent to create a robust implementation plan. The task we're going to plan today is: <task_id>$1</task_id>.
2. When the task-planner agent completes, spawn the task-implementer agent to implement the plan.
3. When the task-implementer agent completes, spawn the task-reviewer agent to review the implementer's work
4. Finally, when the task-reviewer has completed, perform a final check on the git diff vs main to ensure the task has been completed per the requirements laid out in the task's description and implementation plan.

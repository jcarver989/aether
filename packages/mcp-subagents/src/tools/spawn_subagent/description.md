Spawn sub-agents in parallel to perform tasks concurrently.

Takes an array of tasks, each with:
- agent_name: name of the agent from sub-agents directory
- prompt: the task for the agent to perform

All agents execute in parallel. Results are returned when ALL agents complete.
Each agent returns structured output with: summary, artifacts, decisions, next_steps.

Ideal for:
- Parallel codebase exploration
- Concurrent file analysis
- Multi-aspect code review

# Plugins MCP Server
Dynamic plugin system for loading and executing skills and sub-agents.

## Skills
Skills are reusable knowledge blocks loaded from markdown files. Each skill contains specialized knowledge that can be loaded into an agent's context.

**Workflow:**
1. Use `list_skills` to discover available skills
2. Use `get_skills` to load the full content of one or more skills
3. Skills that don't exist are silently skipped

## Sub-Agents
Sub-agents are specialized agents that can be spawned in parallel to perform concurrent tasks. Since you have a finite context window, they are extremely useful to preserve your own context. Spawn sub-agents whenever you have a context-intensive task that's a part of the larger task you're working on (e.g. exploring a codebase to generate a plan). If there are tasks that can be effectively parallelized, you may spawn multiple sub-agents in parallel.


**Workflow:**
1. Use `list_subagents` to discover available agents and their descriptions
2. Use `spawn_subagent` to execute tasks with one or more agents
3. All agents execute in parallel and return results when complete

**Important:** Always call `list_subagents` before `spawn_subagent` to discover available agents and their purposes.

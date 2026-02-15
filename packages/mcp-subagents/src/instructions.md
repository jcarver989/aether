# Sub-Agents MCP Server
System for spawning and managing sub-agents.

## Sub-Agents
Sub-agents are specialized agents that can be spawned in parallel to perform concurrent tasks. Spawn sub-agents via the `spawn_subagent` tool whenever you have a context-intensive task that can be delegated (e.g. exploring a codebase to generate a plan). If there are tasks that can be effectively parallelized, you may spawn multiple sub-agents in parallel.

# Plugins MCP Server
Dynamic plugin system for loading and executing skills and sub-agents.

## Skills
Skills are reusable knowledge blocks loaded from markdown files. Each skill contains specialized knowledge that can be loaded into your context.
You MUST Load ALL skills whose names/descriptions are applicable to your task via the `get_skills` tool.

## Sub-Agents
Sub-agents are specialized agents that can be spawned in parallel to perform concurrent tasks. Spawn sub-agents via the `spawn_subagent` tool whenever you have a context-intensive task that can be delegated (e.g. exploring a codebase to generate a plan). If there are tasks that can be effectively parallelized, you may spawn multiple sub-agents in parallel.

Create or update an agent-authored skill.

Use this when you learn something worth remembering: a convention, pitfall, debugging insight,
or codebase fact. One concept per skill. Keep it specific and actionable.

- If the skill doesn't exist, it will be created with `agent_authored: true`.
- If the skill exists and is agent-authored, its description, tags, and content will be updated.
  Existing helpful/harmful counters are preserved.
- Human-authored skills (without `agent_authored: true`) cannot be overwritten — use this tool
  only for agent notes.

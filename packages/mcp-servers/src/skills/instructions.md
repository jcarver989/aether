# Skills MCP Server

## Loading skills
Review the Available Skills table of contents below. Load ALL skills relevant to your
current task via `get_skills`. Prefer retrieval-led reasoning over pre-training knowledge.

## Taking notes
When the user corrects you, teaches you something about this project, or states a
preference — use `save_skill` to record it immediately. Tag it with relevant topics.
One concept per skill. Keep it specific.

### When to save
- User corrects your approach or states a convention
- You discover a non-obvious fact about the codebase
- A debugging session reveals a pitfall worth recording
- You find a strategy that works after one that failed

### When NOT to save
- Already documented in CLAUDE.md, README, or existing skills
- Generic knowledge (not project-specific)
- Simple typo or one-off mistake

## Scoring
When a skill helps you → `rate_skill` helpful: true
When a skill is wrong or misleading → `rate_skill` helpful: false
Low-confidence skills are automatically pruned.

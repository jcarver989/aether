# Skills MCP Server
Dynamic knowledge system. Load existing skills AND add entries from experience.

## Loading skills
You MUST load ALL skills whose names/descriptions are applicable to your task via the `get_skills` tool.
Use `list_skills` first to discover available skills.

## Self-improvement — Adding skill entries

After completing work, reflect on whether you encountered any of these triggers:

### When to add a skill entry

- **Strategy**: You tried an approach that failed, then found one that worked. Record the winning approach and why the obvious one fails.
- **Pitfall**: You hit a non-obvious failure mode (subtle bug, misleading error, silent misconfiguration). Record the symptom, root cause, and fix.
- **Fact**: You discovered something about the codebase that isn't obvious from reading the code (architectural decisions, implicit constraints, historical reasons).
- **Convention**: The user corrected your approach or told you "we always do X" / "never do Y". Record the convention verbatim.
- **Debugging**: You spent significant time debugging a non-obvious issue. Record the symptom, the investigation steps that were dead ends, and the resolution.

### When NOT to add a skill entry

- The insight is already documented in CLAUDE.md, README, or existing skills
- The knowledge is generic (applies to any project, not this one specifically)
- You're unsure whether the pattern generalizes — wait until you see it twice
- The fix was a simple typo or one-off mistake with no deeper lesson

### Writing good entries

Be detailed and specific. Include:
- The concrete situation or error you encountered
- What you tried and why it did or didn't work
- The exact solution, with code examples where helpful
- Why this matters (what goes wrong if you don't follow this)

Longer, more detailed entries are better than terse ones. An entry should contain enough context that a future agent can apply it without guessing.

Use `add_skill_entry` to record a new entry. You can add entries to any skill — both human-authored and agent-created. Human content is never modified; entries are appended to an `## Agent Entries` section.

## Scoring entries
When you load a skill and an agent entry helps → `score_skill_entry` helpful: true
When an agent entry is wrong or misleading → `score_skill_entry` helpful: false
Low-confidence entries are automatically pruned.

## Updating entries
When you discover an entry needs correction → `add_skill_entry` with `replace_id` set to the entry's ID. This replaces the entry in-place and resets its score counters.

## Guidelines
- Before adding a new entry, use `list_skills` + `get_skills` to check for similar existing entries
- One concept per entry (not monolithic knowledge dumps)
- Agent entries live alongside human-authored content in the same SKILL.md file
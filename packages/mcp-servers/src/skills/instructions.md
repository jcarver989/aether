# Skills MCP Server

## Loading skills
Review the Available Skills table of contents below. Load ALL skills relevant to your
current task via `get_skills`. Prefer retrieval-led reasoning over pre-training knowledge.

## Notes
Use `search_notes` at the start of a task to check for relevant learnings from prior
conversations. Use `save_note` to record insights worth preserving across conversations.

### When to save a note
- User corrects your approach or states a convention
- You discover a non-obvious fact about the codebase
- A debugging session reveals a pitfall worth recording

### Quality bar
- Only save if this will be useful across multiple future conversations
- If the decision is already reflected in committed code, don't save it
- Search for an existing note on this topic before creating a new one — append, don't duplicate
- Prefer fewer, richer topic notes over many atomic ones

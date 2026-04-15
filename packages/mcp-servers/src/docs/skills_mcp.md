MCP server for unified prompt artifacts -- skills, slash commands, and rules.

This server loads prompt artifacts from explicitly configured prompt directories. It exposes tools for loading skills/prompts and managing notes, and implements the MCP prompts protocol so prompts can be invoked as slash commands.

# Construction

```rust,ignore
use mcp_servers::SkillsMcp;

let server = SkillsMcp::new(
    &["/my/project/.aether/skills".into(), "/my/project/.claude/rules".into()],
    "/my/project/.aether/notes".into(),
);

// From CLI args
let server = SkillsMcp::from_args(vec![
    "--dir".into(), "/my/project/.aether/skills".into(),
    "--dir".into(), "/my/project/.claude/rules".into(),
    "--notes-dir".into(), "/my/project/.aether/notes".into(),
]).unwrap();
```

# Tools provided

- **`get_skills`** -- Load prompt files by name. Supports both `<name>/SKILL.md` directories and flat `<name>.md` files.
- **`save_note`** -- Save a note (creates or updates a file in the notes directory).
- **`search_notes`** -- Search saved notes by keyword.

# Prompts

The server also implements the MCP prompts protocol, listing available user-invocable prompts.

# See also

- [`CodingMcp`](crate::CodingMcp) -- Uses prompt catalogs for read-triggered rules via [`PromptRuleMatcher`](crate::coding::prompt_rule_matcher::PromptRuleMatcher).
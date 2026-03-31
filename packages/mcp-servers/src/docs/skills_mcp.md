MCP server for unified prompt artifacts -- skills, slash commands, and rules.

Skills are reusable prompt files stored in the project's `.aether/skills/` directory. This server exposes tools for loading skills and managing notes, and implements the MCP prompts protocol so skills can be invoked as slash commands.

# Construction

```rust,ignore
use mcp_servers::SkillsMcp;

let server = SkillsMcp::new("/my/project/.aether".into());

// From CLI args (e.g. --dir ~/.aether)
let server = SkillsMcp::from_args(vec!["--dir".into(), "/home/user/.aether".into()]).unwrap();
```

# Tools provided

- **`get_skills`** -- Load skill files from the skills directory. Optionally filter by name.
- **`save_note`** -- Save a note (creates or updates a file in the notes directory).
- **`search_notes`** -- Search saved notes by keyword.

# Prompts

The server also implements the MCP prompts protocol, listing available skills as invocable prompts. This allows MCP clients to present skills as slash commands with argument substitution.

# See also

- [`CodingMcp`](crate::CodingMcp) -- Uses the same skill catalog for read-triggered rules via [`PromptRuleMatcher`](crate::coding::prompt_rule_matcher::PromptRuleMatcher).

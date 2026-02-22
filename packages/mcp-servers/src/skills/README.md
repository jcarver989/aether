# SkillsMcp

Slash commands and reusable skill prompts. Skills teach the agent domain-specific knowledge; commands trigger multi-step workflows.

**Flag:** `--dir <path>` (base directory containing `commands/` and `skills/` subdirectories)

## Directory Structure

```
~/.aether/
├── commands/           # Slash commands (markdown files)
│   ├── commit.md
│   ├── review-branch.md
│   └── ...
└── skills/             # Skill directories
    ├── rust/
    │   └── SKILL.md
    ├── frontend-design/
    │   └── SKILL.md
    └── ...
```

## Tools

| Tool | Description |
|------|-------------|
| `list_skills` | List all available skills with their names and descriptions. |
| `get_skills` | Load the full content of one or more skills by name. |

Commands are exposed as **MCP Prompts** (via `list_prompts` / `get_prompt`) rather than tools. This is what powers `/slash-commands` in the TUI.

## Writing a Command

Create a markdown file in `commands/` with YAML frontmatter:

```markdown
---
description: Generate a commit message for staged changes
argument-hint: [optional args]
allowed-tools:
  - Bash
  - Read
  - Grep
---

Your prompt template here. Use $1, $2, etc. for positional arguments.
```

| Frontmatter Field | Description |
|-------------------|-------------|
| `description` | Shown when listing available commands. |
| `argument-hint` | Hint text shown to the user for expected arguments. |
| `allowed-tools` | Restricts which tools the agent can use while executing the command. |

The filename (minus `.md`) becomes the command name: `commands/review-branch.md` -> `/review-branch`.

## Writing a Skill

Create a directory under `skills/` with a `SKILL.md` file:

```markdown
---
name: rust
description: Rust best practices and project conventions
---

# Rust Coding Guidelines

Your skill content here — conventions, patterns, examples, etc.
```

Skills are loaded on-demand when the agent calls `get_skills`. They inject domain knowledge into the agent's context without consuming tokens until needed.

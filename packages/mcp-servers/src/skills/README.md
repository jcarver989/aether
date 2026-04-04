# SkillsMcp

Slash commands and reusable skill prompts. Skills teach the agent domain-specific knowledge; commands trigger multi-step workflows.

**Flag:** `--dir <path>` (base directory containing `commands/` and `skills/` subdirectories)

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

- [Directory Structure](#directory-structure)
- [Tools](#tools)
- [get_skills API](#get_skills-api)
  - [Examples](#examples)
  - [Response Fields](#response-fields)
  - [Security](#security)
- [Writing a Command](#writing-a-command)
- [Writing a Skill](#writing-a-skill)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

## Directory Structure

```
~/.aether/
├── commands/           # Slash commands (markdown files)
│   ├── commit.md
│   ├── review-branch.md
│   └── ...
└── skills/             # Skill directories (multi-file support)
    ├── rust/
    │   ├── SKILL.md        # Primary skill content (required)
    │   ├── traits.md       # Auxiliary file
    │   └── references/
    │       └── REF.md      # Nested auxiliary file
    └── ...
```

## Tools

| Tool | Description |
|------|-------------|
| `get_skills` | Load files from skill directories. Omit `path` for `SKILL.md`, or provide `path` for auxiliary files. |
| `save_note` | Append a learning to a topic-based note file. Notes consolidate learnings by topic. |
| `search_notes` | Search notes by topic name (substring) or tag (exact match). |

Commands are exposed as **MCP Prompts** (via `list_prompts` / `get_prompt`) rather than tools. This is what powers `/slash-commands` in the TUI.

## get_skills API

`get_skills` loads files from skill directories with progressive disclosure:

- Omit `path` → loads `SKILL.md`
- Provide `path` → loads that file relative to the skill root
- When loading `SKILL.md`, the response includes `availableFiles` (manifest of auxiliary files)

### Examples

Load a skill root:
```json
{ "requests": [{ "name": "rust" }] }
```

Load auxiliary files:
```json
{ "requests": [
  { "name": "rust", "path": "traits.md" },
  { "name": "rust", "path": "references/REF.md" }
] }
```

### Response Fields

- `name` — skill name
- `path` — file path (normalized to `SKILL.md` if omitted)
- `content` — file content (null if error)
- `error` — error message if loading failed
- `availableFiles` — auxiliary files in the skill (only for `SKILL.md`)

### Security

Path validation prevents:
- Absolute paths
- Path traversal (`..`)
- Access outside the skill directory
- Directory paths (only files allowed)

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
tags: [rust, testing]
---

# Rust Coding Guidelines

See [traits](./traits.md) for trait conventions.
See [error-handling](./error-handling.md) for error patterns.
```

Skills support multi-file content. The `SKILL.md` is the primary file loaded by `get_skills`. Auxiliary files (like `traits.md`, `error-handling.md`) can be loaded on-demand by providing the `path` parameter.

The agent discovers auxiliary files via the `availableFiles` field when loading `SKILL.md`.

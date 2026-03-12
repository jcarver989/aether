# Implementation Plan: Unify Slash Commands, Skills, and Rules via `SKILL.md`

## Objective

Unify slash commands, skills, and rules under a single prompt artifact while preserving the open skills standard:

- **Artifact format stays standard**: one directory per prompt under `.aether/skills/<name>/SKILL.md`
- **Slash command** = prompt with `user-invocable: true`
- **Skill** = prompt with `agent-invocable: true`
- **Rule** = prompt with `triggers.read` globs
- A single prompt may support any combination of the above

This plan describes the **final desired end state only**.

---

# 1. Final authored model

## Skill root

Project-local prompt artifacts live under:

```text
.aether/skills/<prompt-name>/SKILL.md
```

Supporting files may live alongside `SKILL.md` in the same directory.

## `SKILL.md` frontmatter contract

Use YAML frontmatter with these fields:

```yaml
name: explain-code
description: Explain code with diagrams and analogies
user-invocable: true
agent-invocable: true
argument-hint: "[path]"
tags:
  - teaching
  - rust
triggers:
  read:
    - "packages/**/*.rs"
```

## Semantics

- `name`
  - optional
  - defaults to directory name
  - must be unique within the project catalog
- `description`
  - required for project-authored prompts
  - used for skill/tool discovery and slash command display
- `user-invocable`
  - if `true`, expose via MCP `prompts/list` and `prompts/get`
- `agent-invocable`
  - if `true`, expose via `get_skills`
- `argument-hint`
  - optional UI hint for slash command invocation
- `tags`
  - existing skill metadata; preserve as-is
- `triggers.read`
  - list of project-relative glob patterns
  - if any pattern matches a successfully read file, activate the prompt into runtime context

## Valid prompt configurations

A prompt is valid if at least one of these is true:

- `user-invocable == true`
- `agent-invocable == true`
- `triggers.read` is non-empty

This permits:
- pure skill
- pure slash command
- pure rule
- combined surfaces

---

# 2. Internal architecture

## Canonical runtime model

Introduce a shared internal type for discovered prompt artifacts.

### `PromptSpec`

Create a project-level resolved type with the following shape:

- `name: String`
- `description: String`
- `content: String`
- `path: PathBuf` for the skill directory or `SKILL.md`
- `user_invocable: bool`
- `agent_invocable: bool`
- `argument_hint: Option<String>`
- `tags: Vec<String>`
- `triggers: PromptTriggers`
- `agent_authored: bool`
- `helpful: u32`
- `harmful: u32`

### `PromptTriggers`

- `read_globs: Vec<String>`

## Catalog

Add a `PromptCatalog` that:

- discovers `.aether/skills/*/SKILL.md`
- parses frontmatter + markdown body
- validates names and trigger globs
- exposes filtered views:
  - `user_invocable()`
  - `agent_invocable()`
  - `matching_read_rules(path)`

## Placement

Implement the catalog in `packages/aether-project`, not in `mcp-servers`.

Reason:
- discovery + validation is project configuration logic
- the catalog is needed by both MCP prompt exposure and runtime rule activation

---

# 3. Runtime behaviors by invocation surface

## A. User-invocable prompts: slash commands

### Behavior

Any `PromptSpec` with `user_invocable == true` is exposed as an MCP prompt.

### MCP surface

In `SkillsMcp`:

- `list_prompts` returns all `PromptSpec`s with `user_invocable == true`
- `get_prompt` resolves the selected `PromptSpec` body and applies argument substitution

### Argument model

Keep Aether’s current simple slash argument model:

- `ARGUMENTS` = full raw argument string
- `1`, `2`, `3`, ... = positional arguments

No separate structured argument schema is needed.

### `argument-hint` mapping

Preserve current ACP/Wisp flow and improve it minimally:

- expose a single optional MCP prompt argument named `ARGUMENTS`
- put the `argument-hint` text in that argument’s description
- update `map_mcp_prompt_to_available_command` so:
  - if the prompt has exactly one argument named `ARGUMENTS`
  - and that argument has a description
  - use the description as the command input hint
  - otherwise keep current fallback behavior

This keeps slash command UX high quality without inventing a new transport.

---

## B. Agent-invocable prompts: skills

### Behavior

Any `PromptSpec` with `agent_invocable == true` is exposed through the skills MCP tools.

### MCP tools

- `get_skills` returns only `PromptSpec`s with `agent_invocable == true`
- `save_skill` writes a valid `SKILL.md` artifact
- `rate_skill` preserves existing behavior

### Save defaults

`save_skill` should continue creating standard skill artifacts and should write frontmatter defaults that make the saved artifact a skill:

- `agent-invocable: true`
- `user-invocable: false`
- no triggers by default

This preserves the current self-improvement workflow while fitting the unified model.

### Discovery instructions

`SkillsMcp::build_instructions` should list only `agent_invocable` prompt specs in the “available skills” section.

Pure slash commands and pure rules should not appear there.

---

## C. Auto-triggered prompts: rules

### Behavior

Any `PromptSpec` with `triggers.read` participates in rule activation.

When the agent successfully reads a file:
1. normalize the read path relative to project root
2. find all `PromptSpec`s whose `read_globs` match
3. activate any matching prompts not already active in the session
4. inject their content into system context in declaration order

### Trigger source

Only successful `read_file` tool calls should trigger rules.

Do **not** trigger on:
- `grep`
- `find`
- `list_files`
- failed reads

This keeps rule activation deterministic and easy to reason about.

### Session behavior

- rules activate once per session
- reading additional matching files does not duplicate them
- `/clear` removes all activated rules
- after `/clear`, reading a matching file activates the rule again

---

# 4. Context and agent runtime design

## Core requirement

Triggered rules must become **system-level context**, not user content.

## Runtime state

Add to agent runtime state:

- `active_prompt_rules: HashSet<String>` or similar
- `base_system_messages: Vec<ChatMessage>` captured at startup

## Startup behavior

At agent startup:
- build the normal system prompt from existing configured prompts
- store that initial system message set as `base_system_messages`

## Rule activation behavior

When a rule activates:
- append a new `ChatMessage::System` containing that prompt’s rendered body
- record the prompt name in `active_prompt_rules`

Multiple active rules should produce multiple system messages in stable order.

## Clear-context behavior

Current clear behavior preserves all system messages. That is not correct for triggered rules.

Change clear behavior so that:
- the context is reset to `base_system_messages`
- all dynamic rule system messages are removed
- `active_prompt_rules` is cleared

This is the cleanest and most predictable implementation.

---

# 5. Code changes by package

## `packages/aether-project`

## Add prompt catalog support

### New module(s)

Add a prompt catalog module, e.g.:

- `src/prompt_catalog.rs`

### Responsibilities

- discover `.aether/skills/*/SKILL.md`
- parse frontmatter into resolved `PromptSpec`
- validate:
  - unique names
  - required description
  - valid trigger globs
  - at least one activation surface
- expose catalog queries

### Errors

Extend project error types with prompt-specific errors:

- duplicate prompt name
- missing description
- invalid trigger glob
- invalid prompt name
- prompt with no activation surface

---

## `packages/mcp-servers/src/skills/skill_file.rs`

## Extend frontmatter parsing

Expand `SkillsFrontmatter` to include:

- `name: Option<String>`
- `user_invocable: bool`
- `agent_invocable: bool`
- `argument_hint: Option<String>`
- `triggers: Option<...>`

Use serde aliases so YAML uses kebab-case:
- `user-invocable`
- `agent-invocable`
- `argument-hint`

Keep existing fields unchanged:
- `description`
- `tags`
- `agent_authored`
- `helpful`
- `harmful`

## Metadata loading

`SkillMetadata` should be upgraded or replaced so the MCP layer can determine:
- whether a prompt is user-invocable
- whether it is agent-invocable
- whether it has rule triggers

---

## `packages/mcp-servers/src/skills/server.rs`

## Replace commands/skills split with unified prompt catalog

Remove the conceptual split between:
- `commands_dir`
- `skills_dir`

`SkillsMcp` should load a unified prompt catalog and then expose filtered views:

- MCP prompts => `user_invocable`
- skills tools => `agent_invocable`

### `list_prompts`

Return `PromptSpec`s with `user_invocable == true`.

### `get_prompt`

Load selected `PromptSpec`, substitute arguments, return a single text prompt message.

### `get_skills`

Return `PromptSpec`s with `agent_invocable == true`.

### `save_skill` / `rate_skill`

Preserve current behavior but operate on the unified frontmatter model.

---

## `packages/aether-cli/src/acp/mappers.rs`

## Improve slash hint extraction

Update `map_mcp_prompt_to_available_command` to support the unified prompt format cleanly:

- if prompt has one `ARGUMENTS` argument and that argument has a description, use the description as the hint
- otherwise retain existing fallback logic

No other ACP surface changes should be required.

---

## `packages/aether-core/src/core/agent.rs`

## Add rule activation

Hook rule activation into successful tool completion.

### Trigger condition

When a completed tool call is:
- `read_file`
- or namespaced equivalent such as `coding__read_file`

and it succeeded, extract the file path from tool call arguments and evaluate rule matches.

### Activation

For each newly matched prompt:
- append a system message
- mark as active

### Reset

On clear context:
- rebuild context from `base_system_messages`
- clear `active_prompt_rules`

---

# 6. Validation rules

The catalog must enforce all of the following:

## Name rules

- derived from frontmatter `name` or directory name
- unique within the project
- normalized and validated against the same constraints used elsewhere for skill names

## Description

- required
- trimmed, non-empty

## Surface validity

Reject prompts with:
- `user_invocable == false`
- `agent_invocable == false`
- empty `triggers.read`

## Trigger globs

- must compile as valid glob patterns
- are evaluated relative to project root

## Visibility rules

- pure rule: valid, but not listed as skill or slash command
- pure slash command: valid, only shown via MCP prompts
- pure skill: valid, only returned by `get_skills`
- dual-use prompt: valid, appears in both surfaces

---

# 7. Test plan

## `packages/aether-project`

Add catalog tests for:

- user-only prompt
- agent-only prompt
- rule-only prompt
- dual-use prompt
- duplicate names
- invalid trigger glob
- missing description
- no activation surface

## `packages/mcp-servers`

Add MCP tests for:

- `list_prompts` returns only `user_invocable` prompts
- `get_prompt` renders a skill-backed prompt
- `get_skills` returns only `agent_invocable` prompts
- dual-use prompt appears in both surfaces
- pure rule appears in neither surface
- `save_skill` writes valid extended frontmatter

## `packages/aether-cli`

Add slash command tests for:

- `argument-hint` is surfaced correctly through MCP prompt arguments
- dual-use prompt maps to an available slash command correctly

## `packages/aether-core`

Add agent runtime tests for:

- successful read of matching file activates rule
- non-matching read does not activate rule
- same rule activates only once
- multiple rules activate in catalog order
- clear context removes activated rules
- reading again after clear reactivates them

Prefer integration-style tests that validate actual context state, not mocks.

---

# 8. Acceptance criteria

The implementation is complete when all of the following are true:

1. A project author can create `.aether/skills/<name>/SKILL.md` with frontmatter and have it discovered automatically.
2. A prompt with `user-invocable: true` appears as a slash command.
3. A prompt with `agent-invocable: true` appears via `get_skills`.
4. A prompt with both flags appears in both places.
5. A prompt with `triggers.read` activates automatically when a matching file is successfully read.
6. Triggered prompts are injected as system context, not user content.
7. Triggered prompts activate once per session and are removed by `/clear`.
8. `save_skill` and `rate_skill` continue to work using the standard `SKILL.md` artifact format.
9. The implementation does not require a second prompt authoring system in `.aether/settings.json`.

---

# 9. Design constraints to preserve

These are non-negotiable in the implementation:

- **Do not replace the open skill standard artifact format**
- **Do not introduce a second canonical prompt registry in settings.json**
- **Do unify all three behaviors behind one internal prompt model**
- **Do keep rules as prompt artifacts, not a separate rule engine**
- **Do keep runtime behavior simple and deterministic**

---

# 10. Recommended execution order

1. Extend `SKILL.md` frontmatter parsing
2. Build `PromptSpec` + `PromptCatalog` in `aether-project`
3. Refactor `SkillsMcp` to consume the catalog
4. Update ACP slash hint mapping
5. Implement runtime read-triggered activation
6. Add full validation and integration tests

---

## Final architecture summary

The final system should have exactly one authored prompt artifact:

- `SKILL.md`

and exactly one internal concept:

- `PromptSpec`

with three independent behaviors controlled by frontmatter:

- `user-invocable` → slash command
- `agent-invocable` → skill
- `triggers.read` → rule

That gives the symmetry you want without breaking the open skill standard.

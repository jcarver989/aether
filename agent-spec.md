# Shared AgentSpec + `AgentBuilder::from_spec` Implementation Plan

## Goal

Make **AgentSpec** the canonical abstraction for authored agent definitions across the stack.

This plan intentionally skips incremental compatibility work and goes straight to the long-term shape:

- **Main agent** sessions are launched from a concrete AgentSpec-derived runtime input
- **Mode** = user-invocable AgentSpec
- **Sub-agent** = agent-invocable AgentSpec
- **Headless** uses the same prompt/MCP resolution path as the rest of the runtime
- `AgentBuilder::from_spec(...)` becomes the canonical way to construct authored agents in `aether-core`

This replaces the current split where:
- ACP modes are lightweight `settings.json` model presets
- sub-agents are loaded from `sub-agents/<name>/AGENTS.md`
- the main agent runtime is assembled separately from authored specs
- headless prompt/MCP composition is assembled separately again

## Architectural Ownership

Split responsibilities cleanly:

- **`aether-core` owns runtime/domain primitives**
  - `AgentSpec`
  - `AgentBuilder::from_spec(...)`
  - existing prompt/runtime primitives
- **`aether-project` owns project-local `.aether/` semantics**
  - `.aether/settings.json` DTOs
  - parsing and validation
  - path normalization relative to project root
  - resolved catalog/runtime input types
  - MCP precedence resolution
  - runtime fingerprint generation
- **`aether-cli` consumes `aether-project` for ACP and headless flows**
- **`mcp-servers` consumes `aether-project` for standalone stdio sub-agent serving or accepts already-resolved data for tests/internal callers**

This avoids:
- duplicating `.aether/settings.json` ownership across crates
- making `aether-core` responsible for project filesystem/config semantics
- introducing a crate dependency cycle by defining shared resolved-catalog types only in `aether-cli`
- letting sub-agent execution drift from main/headless session behavior
- making the sub-agents MCP server usable only from within Aether

## Why `aether-project`

Use a new shared crate named **`aether-project`** rather than `aether-config`.

Rationale:
- the responsibility is specifically **project-local `.aether/` behavior**, not generic app configuration
- the crate can grow naturally to include other project-scoped loading rules later
- the name makes it clear that these semantics are rooted at a workspace/project directory, not user/home configuration

## Shared Loader Strategy

Because both `aether-cli` and `mcp-servers` need to resolve project-authored agents, agent loading should be shared rather than owned only by `aether-cli`.

Planned direction:
- add `packages/aether-project`
- that crate parses project `.aether/settings.json`
- that crate validates and resolves it into `aether-core` runtime types plus project-derived runtime inputs
- `aether-cli` uses it for ACP and headless
- `mcp-servers` uses it for standalone stdio sub-agent serving

This preserves standalone `mcp-servers-stdio subagents` usability while still centralizing authored agent definitions in one file.

## Sub-agents MCP Server Construction

`SubAgentsMcp` should support both standalone loading and injected construction for tests/internal callers.

Suggested direction:

```rust
impl SubAgentsMcp {
    pub fn from_project_root(project_root: PathBuf) -> Result<Self, SubAgentsError>;
    pub fn new(catalog: ResolvedAgentCatalog, project_root: PathBuf) -> Self;
}
```

Behavior:
- `from_project_root(...)` uses `aether-project` to load `.aether/settings.json`
- `new(...)` accepts already-resolved catalog data for tests or internal callers that already have it
- both paths produce identical runtime behavior

This means:
- external non-Aether agents can still use the sub-agents MCP server over stdio
- Aether internals can still inject resolved state when convenient
- there is one authored source of truth, but not one privileged loader

## Why use `.aether/settings.json` with `agents: []`

We are explicitly choosing a centralized `agents: []` array in project `.aether/settings.json` as the authored configuration surface.

Rationale:
- more discoverable than scanning directories for per-agent config files
- easier to edit than frontmatter-based markdown config
- avoids the awkwardness of markdown files acting as pure config containers
- keeps all authored agent config in one obvious place
- keeps MCP configuration centralized instead of scattering it across agent directories

Prompt content remains file-backed and reusable through explicit prompt references.

## Settings Boundary

Project `.aether/settings.json` is the only authored agent registry in this change.

Rules:
- project `.aether/settings.json` owns authored `agents`, inherited `prompts`, and inherited `mcpServers`
- user-level settings do **not** contribute authored agents in this design
- existing user-level preference storage may continue for non-authored preferences, but that is separate from the authored agent registry
- there is no merging of project-authored agents with user/home-authored agents in this migration

This keeps authored agent behavior deterministic and project-local.

## Missing Settings Semantics

Missing project settings must be handled explicitly.

### If `.aether/settings.json` is absent

For `aether-project` resolution:
- this is **valid**, not an error
- the resolved catalog contains:
  - no authored agents
  - no inherited top-level prompts
  - no inherited top-level MCP config

### Surface behavior

- **ACP / headless:** operate normally with no authored agents and no inherited project prompts/MCP config
- **Standalone sub-agents MCP server:** start successfully and expose no sub-agents if the project has no authored agents
- **Malformed settings:** still fail clearly

This preserves robustness while keeping authored-config errors fail-fast.

## Scope

In scope:
- New shared `AgentSpec` domain model in `aether-core`
- New `aether-project` crate for `.aether/settings.json` schema, parsing, validation, and resolution
- Reuse of `aether-core::core::Prompt` for AgentSpec prompt composition
- Centralized top-level and per-agent MCP config references in `.aether/settings.json`
- `AgentBuilder::from_spec(...)` in `aether-core`
- Migration of ACP modes to AgentSpecs
- Migration of sub-agents to AgentSpecs
- Main-agent session creation using AgentSpec-derived runtime inputs instead of ad hoc mode/model wiring
- Headless prompt/MCP resolution migration onto the same shared path
- Removal of `sub-agents/.../AGENTS.md`
- Session persistence updates needed to restore mode-derived prompt/tool behavior
- Shared MCP builder/runtime wiring so ACP, headless, and sub-agents stop duplicating setup logic
- Standalone stdio support for sub-agents MCP server using the shared loader from project root

Out of scope:
- Backwards compatibility with old mode schema
- Backwards compatibility with `sub-agents/<name>/AGENTS.md`
- User-level global agent-spec directories
- Editing AgentSpecs from the UI
- Unifying slash commands and skills in this change
- Live mutation of prompt/tool surfaces inside an already-running session
- A second non-settings-based authored-agent format for `mcp-servers`

## Design Principles

1. **AgentSpec is the canonical authored runtime/domain artifact.**
2. **Invocation surface is metadata, not a separate model.**
3. **Prompt composition should be reusable and explicit.**
4. **Reuse existing `Prompt` abstractions instead of inventing a parallel prompt-source enum.**
5. **Centralize authored agent config in project `settings.json`.**
6. **`aether-core` owns runtime primitives; `aether-project` owns project parsing/resolution.**
7. **`AgentBuilder::from_spec(...)` is the canonical authored-agent construction path.**
8. **Use strongly typed models in AgentSpec.** Do not store raw model strings in the domain model.
9. **Mode** and **sub-agent** remain UX/API labels only.
10. **No dual support path.** Migrate directly.
11. **Fail fast on authored-config errors.** Invalid authored entries should not silently disappear.
12. **Preserve standalone MCP server usability.** Shared loading is preferable to injection-only construction.
13. **Keep runtime simple:**
    - load project settings
    - resolve settings into typed runtime objects
    - spawn/use

## Canonical Runtime Data Model (`aether-core`)

Add a small AgentSpec module in `aether-core` that owns runtime/domain types only.

Suggested location:
- `packages/aether-core/src/agent_spec.rs`
  or
- `packages/aether-core/src/agent_spec/mod.rs`

Suggested types:

```rust
use crate::core::Prompt;
use llm::{ReasoningEffort, catalog::LlmModel};
use std::path::PathBuf;

pub struct AgentSpec {
    pub name: String,
    pub description: String,
    pub model: LlmModel,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub prompts: Vec<Prompt>,
    pub agent_mcp_config_path: Option<PathBuf>,
    pub exposure: AgentSpecExposure,
}

pub struct AgentSpecExposure {
    pub user_invocable: bool,
    pub agent_invocable: bool,
}
```

Notes:
- `name` is the canonical lookup key
- `model` is strongly typed
- `reasoning_effort` is strongly typed
- `prompts` reuses `aether-core::core::Prompt`
- `agent_mcp_config_path` represents only the agent-local override, not inherited settings-level MCP
- `AgentSpec` is a resolved runtime type, not a raw settings DTO
- validation happens before constructing these runtime types
- user-facing display text can be derived later from `name`

### Why only `AgentSpec` lives in `aether-core`

`ResolvedAgentCatalog` and `AgentRuntimeInputs` should **not** live in `aether-core` because they are project-derived, not purely domain-level.

They depend on:
- project root
- inherited project settings
- `cwd/mcp.json` fallback semantics
- project-specific fingerprint rules
- authored config resolution policy

Those belong in `aether-project`, not `aether-core`.

### Authored vs runtime-owned prompt variants

For authored AgentSpecs resolved from settings, `prompts` should only contain authored-safe prompt variants:
- `Prompt::PromptGlobs { patterns: vec![single_entry], cwd: project_root }`

Runtime-owned prompts remain outside authored settings:
- `Prompt::SystemEnv`
- `Prompt::McpInstructions`

## Shared Project-Config Layer (`aether-project`)

Implement shared settings loading in `aether-project`.

Suggested direction:
- define settings-facing DTOs there
- parse project `.aether/settings.json` there
- resolve settings into concrete `aether_core::agent_spec::AgentSpec` values there
- own the project-derived resolved catalog and runtime input types there
- keep ACP-specific or MCP-server-specific presentation logic out of this layer

This crate should be the only place that knows how project-authored agent config is parsed from disk.

## Project-Derived Runtime Types (`aether-project`)

Suggested types:

```rust
use aether_core::agent_spec::AgentSpec;
use llm::{ReasoningEffort, catalog::LlmModel};
use std::path::{Path, PathBuf};

pub struct ResolvedAgentCatalog {
    pub project_root: PathBuf,
    pub inherited_mcp_config_path: Option<PathBuf>,
    pub specs: Vec<AgentSpec>,
}

pub struct AgentRuntimeInputs {
    pub spec: AgentSpec,
    pub effective_mcp_config_path: Option<PathBuf>,
    pub fingerprint: String,
}

pub fn load_resolved_agent_catalog(project_root: &Path) -> Result<ResolvedAgentCatalog, SettingsError>;
```

Notes:
- the catalog owns project-relative/inherited resolution context
- runtime inputs represent one execution-ready view of a spec in a project
- fingerprinting is owned here because it depends on project-derived resolution behavior

## Settings Schema (`aether-project`)

The canonical authored source becomes project `.aether/settings.json`.

Suggested top-level shape:

```json
{
  "prompts": ["SYSTEM.md", "AGENTS.md"],
  "mcpServers": ".aether/mcp/default.json",
  "agents": [
    {
      "name": "planner",
      "description": "Planner optimized for decomposition and sequencing",
      "model": "anthropic:claude-sonnet-4-5",
      "reasoningEffort": "high",
      "userInvocable": true,
      "agentInvocable": true,
      "prompts": [
        ".aether/prompts/planner.md",
        ".aether/prompts/shared/decomposition.md"
      ],
      "mcpServers": ".aether/mcp/planner.json"
    }
  ]
}
```

Rules:
- top-level `prompts` are inherited by **all** agents
- top-level `mcpServers` is inherited by **all** agents as the default file-backed MCP config source
- `agents` is the canonical authored agent registry
- each agent entry defines explicit prompt references via `prompts: []`
- agent-entry `mcpServers` is optional and points to a valid `mcp.json` file
- there is no implicit prompt-body loading from markdown
- `reasoningEffort` follows existing camelCase settings conventions
- `userInvocable` and `agentInvocable` default to `false`
- `agents[].name` must be unique within the file

## Prompt Composition Rules

Prompt composition is explicit-only for authored AgentSpecs.

### How settings entries map to `Prompt`

Each authored prompt entry becomes **one** `Prompt::PromptGlobs` value.

That means:
- top-level `prompts` entries become inherited `Prompt::PromptGlobs` values
- agent-level `prompts` entries become agent-specific `Prompt::PromptGlobs` values
- do **not** collapse the entire authored prompt array into a single `Prompt::PromptGlobs`

### Ordering rules

For all agents:
1. inherited top-level `settings.json.prompts` entries, in author order
2. runtime-owned prompts:
   - `Prompt::SystemEnv`
   - `Prompt::McpInstructions(...)`
3. agent-specific `agent.prompts` entries, in author order

Within a single prompt entry:
- if the entry is a glob, matches are expanded in lexicographically sorted order

### Validation and resolution timing

Be explicit about when glob validation and resolution occur.

Rules:
- settings resolution validates that every prompt entry currently resolves to at least one file
- runtime input creation resolves the concrete current file matches used for that execution
- fingerprints are computed from the concrete resolved prompt file contents for that execution, not only from the glob strings
- `Prompt::PromptGlobs` may remain the runtime representation, but execution-time fingerprinting must reflect the actual resolved files

Additional validation rules:
- top-level `prompts` may be empty
- each agent entry `prompts` may be empty only if inherited top-level `prompts` is non-empty
- overall, every authored agent must have at least one inherited-or-local prompt entry after resolution
- a zero-match prompt entry is a validation error
- there is no markdown-body fallback or implicit loading

This means authored prompt inclusion is always explicit and visible in `settings.json` while still allowing runtime fingerprinting to reflect real resolved content.

## MCP Configuration Rules

Agent MCP configuration is centralized in project `.aether/settings.json`.

### Field shape

For simplicity, `mcpServers` should be a path to a valid `mcp.json` file, not an inline MCP server definition.

That applies to:
- top-level `settings.json.mcpServers`
- per-agent `settings.json.agents[].mcpServers`

### Effective config selection

Among file-backed MCP config sources, precedence is:
1. `agent.mcpServers`
2. top-level `settings.json.mcpServers`
3. project `cwd/mcp.json`
4. none

### Runtime behavior

Runtime MCP behavior should be:
- built-in in-memory servers are always registered
- runtime-supplied extra MCP servers are always applied
- exactly one file-backed MCP config source is selected using the precedence above
- if no file-backed config applies, run with built-ins plus runtime extras only

### Fingerprint behavior

Runtime fingerprints must reflect **effective MCP behavior**, not only the selected path.

That means fingerprint generation should include:
- the selected effective file-backed MCP config path, if any
- a hash of the selected MCP config file contents, if any

If the MCP config file contents change without the path changing, restore should still detect drift.

## Filesystem Layout

Settings hold all authored agent metadata and MCP config references. Prompt content remains file-backed.

Example:

```text
.aether/
  settings.json
  prompts/
    planner.md
    shared/
      decomposition.md
  mcp/
    default.json
    planner.json
mcp.json
```

Rules:
- project `.aether/settings.json` is the canonical authored registry for agents
- prompt files may live anywhere under the project, but `.aether/prompts/...` is the recommended convention
- authored agent MCP config files may live anywhere under the project, but `.aether/mcp/...` is the recommended convention
- project fallback MCP config remains `cwd/mcp.json`
- `sub-agents/<name>/AGENTS.md` is removed entirely in this migration

## Settings Parsing and Agent Resolution (`aether-project`)

Implement project `.aether/settings.json` loading in `aether-project`.

### Responsibilities

#### Shared settings module
- define settings-facing DTOs for parsing project `.aether/settings.json`
- parse top-level `prompts`, top-level `mcpServers`, and `agents`
- convert each agent entry `model: String` into typed `LlmModel`
- convert each agent entry `reasoningEffort: String` into typed `ReasoningEffort`
- convert each top-level and agent-level prompt entry into one `Prompt::PromptGlobs` with `cwd` bound to the project root
- normalize top-level and per-agent `mcpServers` paths
- apply inheritance rules and produce concrete `ResolvedAgentCatalog` / `AgentSpec` values
- return actionable validation errors

#### Shared resolver
- load all AgentSpecs from project `.aether/settings.json`
- support lookup by exact name
- filter by exposure surface
- compute `AgentRuntimeInputs` for execution surfaces
- return results in deterministic sorted order

Suggested API shape:

```rust
pub fn load_resolved_agent_catalog(project_root: &Path) -> Result<ResolvedAgentCatalog, SettingsError>;

impl ResolvedAgentCatalog {
    pub fn all(&self) -> &[AgentSpec];
    pub fn get(&self, name: &str) -> Result<&AgentSpec, SettingsError>;
    pub fn user_invocable(&self) -> impl Iterator<Item = &AgentSpec>;
    pub fn agent_invocable(&self) -> impl Iterator<Item = &AgentSpec>;
    pub fn runtime_inputs_for(&self, name: &str, cwd: &Path) -> Result<AgentRuntimeInputs, SettingsError>;
    pub fn runtime_inputs_for_default(
        &self,
        model: LlmModel,
        reasoning_effort: Option<ReasoningEffort>,
        cwd: &Path,
    ) -> AgentRuntimeInputs;
}
```

Loading behavior:
- missing `settings.json` returns an empty valid catalog
- malformed `settings.json` fails load
- any invalid authored agent entry fails load
- duplicate agent names fail load
- results are sorted by `name`

There is no bulk-load behavior that silently skips invalid agents.

## Validation Layers

Distinguish between:

### 1. Authored spec validity

The authored project config is valid if:
- project `.aether/settings.json` parses when present
- every `agents[].name` is non-empty
- every `agents[].name` is unique
- every `description` is non-empty
- every `model` parses into `LlmModel`
- every `reasoningEffort`, if present, parses into `ReasoningEffort`
- every agent has at least one invocation flag set
- inherited top-level `prompts` plus agent-local `prompts` yield at least one prompt entry total for every agent
- every prompt entry resolves to at least one file during validation
- `mcpServers`, if present at either top level or agent level, points to a valid `mcp.json` file path

If any authored agent entry is invalid, catalog resolution fails.

### 2. Surface-specific usability

A valid authored spec may still be unusable in a given runtime.

Examples:
- ACP mode presentation should consider whether the typed `model` is present in `available_models()`
- if the UI supports unavailable reasons, an authored mode may be shown as unavailable rather than deleted from existence
- sub-agent execution should fail clearly if provider creation from the typed model fails

Do not conflate authored validity with environment-specific usability.

## Shared Runtime Resolution Helper (`aether-project`)

To prevent ACP, headless, and sub-agents from drifting again, introduce one shared runtime resolution helper that computes execution-ready inputs.

Responsibilities:
- select effective file-backed MCP config using precedence rules
- carry the concrete `AgentSpec`
- compute a stable fingerprint for session persistence / restore drift detection

Suggested shape:

```rust
impl ResolvedAgentCatalog {
    pub fn runtime_inputs_for(&self, name: &str, cwd: &Path) -> Result<AgentRuntimeInputs, SettingsError>;
    pub fn runtime_inputs_for_default(
        &self,
        model: LlmModel,
        reasoning_effort: Option<ReasoningEffort>,
        cwd: &Path,
    ) -> AgentRuntimeInputs;
}
```

### Fingerprint requirements

The fingerprint should reflect **effective runtime behavior**, not only metadata.

At minimum include:
- spec name
- model
- reasoning effort
- ordered authored prompt entries
- hash of the resolved prompt file contents used for that execution
- effective file-backed MCP config path, if any
- hash of the effective MCP config file contents, if any

If prompt or MCP file contents change without names/paths changing, restore should still detect drift.

## Main Agent Unification

The main agent should use the same authored-spec runtime path as sub-agents.

### Canonical construction path

Introduce:

```rust
impl AgentBuilder {
    pub fn from_spec(
        llm: impl StreamingModelProvider + 'static,
        spec: &AgentSpec,
        base_prompts: Vec<Prompt>,
    ) -> Self
}
```

Behavior:
- start with the provided `base_prompts`
- append `spec.prompts` in stored order
- do not manually concatenate strings outside the builder
- preserve existing `Prompt` composition semantics

### Base prompts for all agents

All agents, including the main agent, sub-agents, and the synthesized no-mode default, inherit top-level settings prompts.

That means the runtime prompt stack for all surfaces is:
1. inherited top-level `settings.json.prompts`
2. runtime-owned prompts (`SystemEnv`, `McpInstructions`)
3. agent-specific prompt entries

This is intentional: a no-mode session is still project-scoped and should inherit project-authored base instructions.

### Runtime-only default spec

To fully unify the main-agent path, treat the main agent as always launching from AgentSpec-derived runtime inputs.

If no authored mode is selected:
- synthesize a runtime-only default AgentSpec in memory
- it has the selected/default typed `LlmModel`
- it inherits top-level prompts if present
- it has no agent-local prompts
- it has no agent-local `agent_mcp_config_path`
- it still resolves effective MCP config through the same shared runtime helper

This keeps the runtime model uniform without requiring a checked-in default agent entry.

## ACP / Mode Integration

Modes become projections of `user_invocable` AgentSpecs.

### Settings changes

Replace the current authored mode shape with the generalized project settings schema.

Meaning:
- top-level `settings.json.prompts` remains the inherited project/base prompt stack
- top-level `settings.json.mcpServers` remains the inherited default file-backed MCP config source
- `settings.json.agents` is the canonical authored agent registry
- old inline mode definitions are removed entirely

### Mode discovery

Modes are discovered from:
- `cwd/.aether/settings.json`
- by loading `agents`
- filtering to `user_invocable = true`
- sorting deterministically by `name`

ACP mode labels remain `Mode`, but the underlying model is AgentSpec.

### Session state

Keep per-session state tied to concrete runtime inputs.

Suggested direction:

```rust
struct SessionConfigState {
    active_model: String,
    pending_model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    selected_mode: Option<String>,
    active_spec: AgentSpec,
    active_spec_fingerprint: String,
}
```

Behavior:
- selecting a mode resolves a concrete AgentSpec and fresh runtime inputs
- selection updates staged model + reasoning
- `active_spec` is always present
  - selected mode -> loaded authored AgentSpec
  - no selected mode -> synthesized default AgentSpec
- `active_spec_fingerprint` tracks the effective runtime configuration for persistence/restore
- selected mode name remains the persisted/displayed value
- prompt/tool changes apply on new session creation or session load, not mid-run

Mode lists can be derived from the shared catalog when needed rather than stored redundantly in every session state.

### Session creation

`Session::new` should accept execution-ready runtime inputs rather than raw prompt overrides.

Suggested direction:

```rust
pub async fn new(
    llm: impl StreamingModelProvider + 'static,
    runtime: AgentRuntimeInputs,
    cwd: PathBuf,
    extra_mcp_servers: Vec<McpServerConfig>,
    restored_messages: Option<Vec<ChatMessage>>,
) -> Result<Self, Box<dyn std::error::Error>>
```

Prompt behavior:
- runtime-owned base prompts are built inside session creation
- `runtime.spec` already contains inherited + agent-local authored prompts
- pass `runtime.spec` into `AgentBuilder::from_spec(...)`

MCP behavior:
- `runtime.effective_mcp_config_path` is the selected file-backed config source
- built-ins and runtime extra MCP servers are always applied separately

### Session persistence

This is required for correctness once modes affect prompts/tools.

Persist at least:

```rust
pub struct SessionMeta {
    pub session_id: String,
    pub cwd: PathBuf,
    pub model: String,
    pub selected_mode: Option<String>,
    pub reasoning_effort: Option<String>,
    pub active_spec_name: String,
    pub active_spec_fingerprint: String,
    pub created_at: String,
}
```

On session load:
- resolve the saved mode/default path against current project `.aether/settings.json`
- recompute runtime inputs and fingerprint
- if the authored spec is missing, invalid, or not restorable, fail clearly
- if the fingerprint does not match, fail clearly rather than silently restoring under changed prompts/tools
- if there was no saved mode, synthesize the runtime default AgentSpec again and compare its recomputed fingerprint

Because backwards compatibility is not required, explicit failure is acceptable.

## Headless Integration

Headless should use the same shared runtime resolution path as ACP and sub-agents.

### Behavior

- headless direct model selection may remain
- headless still loads the resolved project agent catalog to inherit top-level prompts and inherited MCP config consistently
- if no named authored agent is selected, headless uses the synthesized default runtime inputs path
- if `--system-prompt` remains supported, it appends after the shared runtime prompt stack rather than replacing it
- headless file-backed MCP selection uses the same precedence helper as other surfaces

This prevents headless from becoming a second ad hoc agent-construction path immediately after the refactor.

## Sub-agent Integration (`mcp-servers`)

Sub-agents become projections of `agent_invocable` AgentSpecs.

### Discovery ownership

`mcp-servers` should no longer discover agents from `sub-agents/` directories or parse `AGENTS.md` frontmatter.

Instead:
- `mcp-servers` loads the shared resolved project agent catalog from project root when running standalone
- or `mcp-servers` accepts an already-resolved catalog through `SubAgentsMcp::new(...)`
- both paths use the same shared loader/runtime behavior

This keeps stdio usage viable without reintroducing a second authored format.

### Standalone stdio usage

The sub-agents MCP server should remain usable as a stdio server for non-Aether agents.

Suggested direction:
- support a `--project-root <path>` argument, defaulting to `.`
- resolve `.aether/settings.json` from that root via `aether-project`
- expose `agent_invocable` specs as available sub-agents

Optional injected construction for tests/internal callers remains fine, but standalone loading must continue to work without an Aether-specific runtime injector.

### Built-in server wiring

Do not duplicate built-in MCP registration logic inside sub-agent execution.

Instead:
- factor shared built-in MCP builder wiring into one reusable helper
- ACP, headless, and sub-agent execution all use that same helper
- sub-agent execution stops hand-rolling built-in server registration separately

### Server instructions

`SubAgentsMcp::build_instructions()` should list:
- `name`
- `description`

from `agent_invocable` AgentSpecs loaded from the shared catalog.

### Execution

When `spawn_subagent` executes a named agent:
- resolve the AgentSpec by exact name from the catalog
- require `agent_invocable = true`
- compute fresh `AgentRuntimeInputs` for that spec
- create LLM from typed `spec.model`
- use `AgentBuilder::from_spec(...)` with `runtime.spec`
- use `runtime.effective_mcp_config_path` for file-backed MCP selection

### `spawn_mcps`

Keep file-backed MCP spawning path-based for simplicity.

Suggested direction:

```rust
async fn spawn_mcps(
    effective_mcp_config_path: Option<&Path>,
    roots: Vec<PathBuf>,
    project_root: &Path,
) -> Result<McpSpawnResult, String>
```

Behavior:
- always register built-in in-memory servers
- always apply runtime-supplied extra servers if present
- apply the already-selected file-backed config path if one exists
- run with built-ins plus runtime extras only when no file-backed config applies

This removes the executor's dependency on per-agent config directories.

## Recommended Delivery Plan

The overall design should be implemented as a sequence of focused PRs rather than one large change.

This keeps review scope smaller and reduces regression risk while still avoiding compatibility shims.

### PR 1: Core primitives + `aether-project`
- Add `AgentSpec` to `aether-core`
- Add `AgentBuilder::from_spec(...)`
- Add `packages/aether-project`
- Implement `.aether/settings.json` parsing, validation, and resolution there
- Add `ResolvedAgentCatalog` / `AgentRuntimeInputs`
- Add runtime-input and fingerprint tests
- Do **not** switch ACP, headless, or sub-agents yet

### PR 2: Main runtime + headless migration
- Refactor ACP mode handling to use `aether-project`
- Refactor session creation to always use `AgentRuntimeInputs`
- Refactor headless to use the shared runtime path
- Update session persistence to store spec fingerprint
- Update session restore to detect authored-spec drift

### PR 3: Sub-agents MCP migration
- Refactor `SubAgentsMcp` to load from `aether-project`
- Add `SubAgentsMcp::from_project_root(...)`
- Keep optional injected `SubAgentsMcp::new(...)` constructor for tests/internal callers
- Remove `sub-agents/.../AGENTS.md` runtime assumptions
- Switch sub-agent execution onto shared MCP builder/runtime wiring
- Preserve standalone stdio usability

### PR 4: Cleanup and obsolete code removal
- Delete old ACP `Mode` struct and inline mode handling
- Delete old `validated_modes` helpers based on inline settings modes
- Delete sub-agent-specific frontmatter parsing
- Delete duplicated sub-agent MCP setup paths
- Remove any remaining dead assumptions about per-agent directories or old formats

## Testing Plan

Follow repository testing guidance: prefer state-based tests over behavior mocks.

### 1. `aether-project` loader / resolution tests
Add tests for:
- missing `.aether/settings.json` yields a valid empty catalog
- valid agent entry with both exposure flags
- valid user-only spec
- valid agent-only spec
- invalid model string rejected during load
- invalid reasoning-effort string rejected during load
- duplicate agent names rejected during load
- top-level prompts inherited by all agents
- top-level `mcpServers` carried as inherited catalog-level config
- agent-specific `prompts` appended after inherited prompts
- one `Prompt::PromptGlobs` created per authored prompt entry
- glob matches sorted deterministically within each entry
- zero-match prompt entry rejected during load
- missing required fields rejected
- no invocation surface rejected
- malformed settings JSON rejected
- invalid `mcpServers` path rejected
- any invalid agent entry fails catalog load

### 2. Catalog / runtime-input tests
Add tests for:
- loading multiple specs from project `.aether/settings.json`
- deterministic sorting by name
- filtering by `user_invocable`
- filtering by `agent_invocable`
- direct lookup by name returns precise errors
- runtime-input helper selects agent MCP over inherited MCP
- runtime-input helper selects inherited MCP over project `cwd/mcp.json`
- runtime-input helper falls back to `cwd/mcp.json`
- fingerprint changes when prompt file contents change without path changes
- fingerprint changes when MCP config contents change without path changes
- runtime-input helper produces stable fingerprints when inputs are unchanged

### 3. `AgentBuilder::from_spec` tests (`aether-core`)
Add tests for:
- `spec.prompts` are appended after runtime-owned base prompts
- authored prompt entries preserve authored order
- resulting system prompt preserves existing prompt ordering semantics

### 4. ACP mode tests (`aether-cli`)
Update tests to verify:
- mode options come from `settings.json.agents`
- authored-invalid specs are load failures, not silent omissions
- unavailable-but-authored specs follow the chosen ACP presentation policy
- selecting a mode resolves model and reasoning effort from spec
- selected mode display remains stable
- session creation uses inherited + agent-local prompts
- agent-specific MCP overrides inherited top-level MCP
- inherited top-level MCP overrides project `cwd/mcp.json`
- no-mode sessions still create and use synthesized default runtime inputs
- session restore fails clearly when the spec fingerprint changes

### 5. Headless tests (`aether-cli`)
Add/update tests to verify:
- headless inherits top-level prompts through the shared runtime path
- headless uses inherited top-level MCP before `cwd/mcp.json`
- headless no-agent execution uses synthesized default runtime inputs
- `--system-prompt` appends after shared runtime prompts if retained

### 6. Sub-agent MCP tests (`mcp-servers`)
Update tests to verify:
- available sub-agent list comes from shared AgentSpecs loaded from project settings
- missing project settings yields an empty-but-valid sub-agents server
- `SubAgentsMcp::from_project_root(...)` works for standalone stdio-style usage
- optional injected construction works for tests/internal callers
- `spawn_subagent` rejects user-only specs
- `spawn_subagent` uses spec model and composed prompt
- `spawn_subagent` uses agent-specific MCP when present
- `spawn_subagent` falls back to inherited top-level MCP when agent config is absent
- `spawn_subagent` falls back to project `cwd/mcp.json` when neither settings-level config is present
- missing all file-backed MCP config is valid and runs with built-ins only
- sub-agent execution uses shared MCP builder wiring rather than a private duplicate path

### 7. End-to-end integration test
Add at least one integration-style test that creates:

```text
.aether/settings.json
.aether/prompts/planner.md
.aether/prompts/shared/decomposition.md
.aether/mcp/default.json
.aether/mcp/planner.json
mcp.json
```

Then asserts:
- ACP exposes `planner` as a mode when `userInvocable = true`
- sub-agents MCP exposes `planner` as spawnable when `agentInvocable = true`
- headless and ACP both use the same inherited + local prompt semantics
- standalone sub-agents stdio loading from project root uses the same catalog
- agent-specific `mcpServers` overrides top-level settings `mcpServers`
- top-level settings `mcpServers` overrides project `cwd/mcp.json`
- session restore detects authored-spec drift via fingerprint mismatch

## File-Level Change List

### New files
- `packages/aether-core/src/agent_spec.rs` or equivalent small module
- `packages/aether-project/src/lib.rs`
- `packages/aether-project/src/settings.rs`
- `packages/aether-project/src/catalog.rs`
- new tests for parser/catalog/builder/runtime behavior

### Major edits
- `packages/aether-core/src/core/agent_builder.rs`
- `packages/aether-core/src/core/mod.rs`
- `packages/aether-cli/src/acp/model_config.rs`
- `packages/aether-cli/src/acp/session_manager.rs`
- `packages/aether-cli/src/acp/session.rs`
- `packages/aether-cli/src/acp/session_store.rs`
- `packages/aether-cli/src/headless/mod.rs`
- `packages/aether-cli/src/headless/run.rs`
- `packages/mcp-servers/src/setup.rs`
- `packages/mcp-servers/src/subagents/server.rs`
- `packages/mcp-servers/src/subagents/tools/spawn_subagent/mod.rs`
- relevant ACP/Wisp tests for mode rendering and selection

## Acceptance Criteria

This work is complete when all of the following are true:

1. `aether-core` defines the canonical `AgentSpec` type and `AgentBuilder::from_spec(...)`.
2. `aether-project` owns project `.aether/settings.json` schema, parsing, validation, and resolution into runtime-ready project types.
3. Project-derived catalog/runtime-input types live in `aether-project`, not `aether-core`.
4. AgentSpecs are authored centrally in project `.aether/settings.json` under `agents: []`.
5. Missing `.aether/settings.json` is valid and yields an empty project catalog.
6. Project-authored agent resolution is fail-fast: invalid authored entries do not silently disappear.
7. Duplicate agent names are rejected.
8. AgentSpecs reuse `aether-core::core::Prompt` rather than introducing a parallel prompt-source abstraction.
9. Authored AgentSpecs use explicit `prompts: []` references only; there is no implicit prompt-body loading.
10. Both top-level `settings.json.prompts` and top-level `settings.json.mcpServers` are inherited by all agents.
11. Agent entry `mcpServers` points to a valid `mcp.json` file and overrides inherited file-backed MCP config.
12. Main-agent session creation resolves through shared runtime inputs, including the no-mode default case.
13. Headless uses the same prompt/MCP resolution path as other surfaces.
14. Sub-agent discovery and execution are backed by the shared project settings loader/catalog, not bespoke `sub-agents/` disk parsing.
15. The sub-agents MCP server remains usable standalone over stdio from a project root without an Aether-specific runtime injector.
16. Prompt entries map to one `Prompt::PromptGlobs` each, preserve authored order, and reject zero-match entries.
17. File-backed MCP precedence is: agent `mcpServers` → top-level `mcpServers` → project `cwd/mcp.json`.
18. Built-ins and runtime-supplied extra MCP servers are always applied independently of file-backed MCP precedence.
19. Runtime fingerprints reflect effective prompt and MCP file contents, not only names/paths.
20. Session resume restores mode-derived prompt/tool behavior correctly and rejects authored-spec drift via fingerprint mismatch.
21. Existing UX/API labels remain unchanged:
    - ACP/Wisp shows **modes**
    - MCP server exposes **sub-agents**
22. Old inline mode config and old sub-agent directory conventions are removed.

## Suggested Non-Goals for This Work

Do not include these unless implementation is trivial:
- user-level global agent-spec directories
- user-level authored agent merging
- editing AgentSpecs from the UI
- ACP `set_session_mode`
- unifying slash commands and skills under the same authored-spec system
- live prompt/tool mutation inside already-running sessions

## Implementation Notes for the Engineer

- Keep the ownership split strict:
  - `aether-core` owns runtime/domain primitives
  - `aether-project` owns project settings parsing and project-derived resolution
  - `aether-cli` and `mcp-servers` consume the same loader/runtime rules
- Parse model strings and reasoning-effort strings exactly once during settings resolution
- Convert each authored prompt entry into one `Prompt::PromptGlobs`
- Preserve author order for prompt entries; do not collapse them into one prompt object
- Within a single prompt entry, sort glob matches deterministically
- Normalize and validate `mcpServers` paths during settings resolution
- Keep lookup exact by spec name; derive display text separately if needed
- Preserve deterministic ordering everywhere a default/first mode matters
- Persist a spec fingerprint so session restore can detect drift in prompts/tools/config
- Compute fingerprints from effective resolved prompt/MCP file contents, not only from references
- Preserve standalone sub-agents MCP stdio usability by loading from project root through `aether-project` rather than requiring Aether-only injection
- Since compatibility is not required, remove obsolete code instead of adding fallback branches

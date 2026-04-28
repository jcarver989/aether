# Inline Agent and MCP Settings Plan

## Overview

### Problem statement

The current Aether CLI resolves authored agents primarily from `cwd/.aether/settings.json` and resolves MCP server config from agent/settings refs or `cwd/mcp.json`. That makes the TypeScript SDK awkward in host applications whose `cwd` is not an Aether project: the SDK can choose `--model` and can pass ACP `mcpServers`, but it cannot provide the full agent settings surface (prompt stack, tool filters, named modes, inherited prompts/MCP refs) without asking the caller to create project files in `cwd`.

There are existing unstaged changes in this checkout that must be preserved. They are unrelated but adjacent MCP/LSP work:

- `packages/aether-lspd/src/client.rs`: daemon ready/reaper refactor and tests.
- `packages/mcp-servers/src/coding/mod.rs`, `src/bin/stdio.rs`, `src/setup.rs`, `src/lib.rs`, `README.md`: new `--disable-lsp`/`LspIntegration` support for the coding MCP server.

This plan extends the configuration pipeline so CLI callers and the TypeScript SDK can provide agent settings and MCP config explicitly, without requiring `cwd/.aether/settings.json` and/or `cwd/mcp.json`.

### Success criteria and acceptance conditions

- `aether acp`, `aether headless`, and `aether show-prompt` can load agent settings from a CLI-provided JSON string or explicit settings file outside `cwd/.aether/settings.json`.
- CLI-provided settings replace project `.aether/settings.json` for that process/run; the current project-file behavior remains the default when no CLI settings source is provided.
- Agent prompt settings support inline prompt text as well as existing prompt-file/glob entries, so SDK callers can provide instructions without writing prompt files.
- `aether headless`, `aether acp`, and `aether show-prompt` can layer MCP configuration from CLI-provided JSON strings, avoiding `cwd/mcp.json` for non-ACP callers and non-SDK CLI use.
- The TypeScript SDK exposes typed options for inline Aether settings and serializes them to the new CLI option when it spawns `aether acp`.
- SDK callers can start a session from a `cwd` with no `.aether/settings.json` and no `mcp.json` by passing SDK options only.
- Existing unstaged LSP/MCP changes are not reverted or overwritten.

## Technical Approach

### High-level architectural decisions

1. **Reuse the existing settings schema and resolver instead of creating a second agent-config model.**
   Add alternate settings sources to `aether-project`, then keep producing the existing `AgentCatalog`/`AgentSpec` runtime types.

2. **Add a prompt-entry enum to settings.**
   Existing settings store `prompts: Vec<String>` where each string is a path/glob. Replace that DTO field with a backwards-compatible untagged enum that accepts:
   - a string path/glob, preserving current JSON;
   - an object with inline text, e.g. `{ "text": "You are a helpful reviewer." }`.

3. **Use explicit CLI settings-source options.**
   Add common options to ACP/headless/show-prompt:
   - `--settings-json <JSON>`: inline `.aether/settings.json` equivalent.
   - `--settings-file <PATH>`: explicit settings file, not necessarily under `.aether/`.
   These should conflict with each other. If neither is provided, keep loading `cwd/.aether/settings.json`.

4. **Use explicit CLI MCP JSON layers.**
   Add repeatable `--mcp-config-json <JSON>` alongside existing `--mcp-config <PATH>`. The JSON shape should match `mcp.json` / `RawMcpConfig` (`servers` or `mcpServers`). When any CLI MCP layer is present, it acts as the existing CLI override and replaces `AgentSpec.mcp_config_refs`; JSON layers are loaded after path layers so later CLI values win.

5. **Expose the same surface in the TypeScript SDK.**
   Add `settings?: AetherSettings` to `AetherSessionOptions`, serialize with `JSON.stringify`, and pass it as `--settings-json`. Keep existing `tools` and `externalMcpServers` behavior, because those already avoid `mcp.json` by using ACP `mcpServers`.

6. **Do not add new dependencies.**
   Rust already has `serde`, `serde_json`, `clap`, and `tempfile`; TypeScript already has the needed runtime/types.

### Key technical considerations and trade-offs

- **CLI arg length:** `--settings-json` is simple and file-free, but long inline prompts can hit OS argument length limits. `--settings-file` is the escape hatch for large configs while still not requiring `cwd/.aether/settings.json`.
- **Prompt validation:** Existing path/glob prompt entries should still be validated at catalog load. Inline text entries should be trimmed/validated as non-empty and converted directly to `Prompt::text`.
- **Precedence:** CLI settings should be a replacement source, not a merge with `.aether/settings.json`. This keeps behavior easy to reason about for SDK callers.
- **MCP factories:** Inline MCP JSON that uses `"in-memory"` built-in servers needs the normal `McpBuilderExt::with_builtin_servers(...)` factories registered before parsing. Add a builder method that parses JSON after factories are registered, rather than pre-parsing in the CLI layer.
- **ACP per-session MCP servers:** SDK `externalMcpServers` and `tools` continue to flow through ACP `newSession.mcpServers`; the new CLI `--mcp-config-json` is mainly for CLI users and for future SDK users who need Aether-native `mcp.json` features such as `in-memory` built-ins.

## Implementation Steps

1. **Add settings source types in `aether-project`.**
   - Modify `packages/aether-project/src/settings.rs`.
   - Add:
     ```rust
     #[derive(Debug, Clone)]
     pub enum AgentCatalogSource {
         ProjectFiles,
         Settings(Settings),
         SettingsFile(PathBuf),
     }
     ```
   - Add `pub fn load_agent_catalog_from_source(project_root: &Path, source: AgentCatalogSource) -> Result<AgentCatalog, SettingsError>`.
   - Keep `load_agent_catalog(project_root)` as a thin wrapper around `AgentCatalogSource::ProjectFiles`.
   - For `SettingsFile`, read/parse that path and call existing `resolve_settings(project_root, settings)`.
   - Update `packages/aether-project/src/lib.rs` to export `AgentCatalogSource` and `load_agent_catalog_from_source`.

2. **Add inline prompt support to settings.**
   - In `packages/aether-project/src/settings.rs`, introduce:
     ```rust
     #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
     #[serde(untagged)]
     pub enum PromptEntry {
         Path(String),
         Text { text: String },
     }
     ```
   - Change `Settings.prompts` and `AgentEntry.prompts` from `Vec<String>` to `Vec<PromptEntry>`.
   - Update `validate_prompt_entries`, `validate_prompt_entry`, `build_inherited_prompts`, and prompt construction in `resolve_agent_entry`:
     - `PromptEntry::Path(pattern)` uses current glob validation and `Prompt::from_globs` behavior.
     - `PromptEntry::Text { text }` validates `text.trim()` is non-empty and produces `Prompt::text(text.trim())` or preserves exact text if whitespace is meaningful; choose trim for validation but keep original text for prompt content.
   - Preserve current string prompt JSON compatibility via the untagged enum.

3. **Add a common CLI settings-args helper.**
   - Create a small helper module, e.g. `packages/aether-cli/src/config_args.rs`.
   - Define:
     ```rust
     #[derive(Clone, Debug, Default, clap::Args)]
     pub struct SettingsSourceArgs {
         #[arg(long = "settings-json", conflicts_with = "settings_file")]
         pub settings_json: Option<String>,
         #[arg(long = "settings-file", conflicts_with = "settings_json")]
         pub settings_file: Option<PathBuf>,
     }
     ```
   - Add a method `into_catalog_source(self) -> Result<AgentCatalogSource, CliError>` that parses `settings_json` into `Settings` with `serde_json::from_str`, wraps `settings_file`, or returns `ProjectFiles`.
   - Use `CliError::AgentError` for malformed settings JSON with an actionable message.

4. **Thread settings source through headless and show-prompt.**
   - Modify `packages/aether-cli/src/headless/mod.rs`:
     - Add `#[command(flatten)] pub settings_source: SettingsSourceArgs` to `HeadlessArgs`.
     - Change `resolve_spec(...)` to accept an `AgentCatalogSource` or `SettingsSourceArgs` and call `load_agent_catalog_from_source(&cwd, source)`.
   - Modify `packages/aether-cli/src/show_prompt/mod.rs` and `src/show_prompt/run.rs` similarly.
   - Keep existing behavior for callers with no new flags.

5. **Thread settings source through ACP startup.**
   - Modify `packages/aether-cli/src/acp/mod.rs`:
     - Add `#[command(flatten)] pub settings_source: SettingsSourceArgs` to `AcpArgs`.
     - Convert it in `run_acp` and store it in `SessionManagerConfig`.
   - Modify `packages/aether-cli/src/acp/session_manager.rs`:
     - Add `catalog_source: AgentCatalogSource` to `SessionManager` and `SessionManagerConfig`.
     - Make `load_mode_catalog(&self, cwd)` use `load_agent_catalog_from_source(cwd, self.catalog_source.clone())` instead of `load_agent_catalog(cwd)`.
     - Ensure `new_session` and `load_session` use the same source.

6. **Add MCP JSON loading to `McpBuilder`.**
   - Modify `packages/aether-core/src/mcp/mcp_builder.rs`.
   - Add:
     ```rust
     pub async fn from_json_strs(mut self, jsons: &[String]) -> Result<Self, ParseError> { ... }
     pub async fn from_raw_config(mut self, raw: RawMcpConfig) -> Result<Self, ParseError> { ... }
     ```
   - Implement `from_raw_config` by calling `raw.into_configs(&self.factories).await?` and extending `self.mcp_configs`.
   - Implement `from_json_strs` by parsing each string with `RawMcpConfig::from_json` and applying in order, so later JSON layers override earlier layers via the `RawMcpConfig` merge semantics or by sequential extension consistent with existing behavior.

7. **Add CLI `--mcp-config-json` and runtime threading.**
   - Add `pub mcp_config_jsons: Vec<String>` to:
     - `HeadlessArgs` in `packages/aether-cli/src/headless/mod.rs`.
     - `PromptArgs` in `packages/aether-cli/src/show_prompt/mod.rs`.
     - `AcpArgs` in `packages/aether-cli/src/acp/mod.rs` if ACP startup-level MCP JSON should be supported.
   - Modify `RunConfig`, `Session::new`, and/or `RuntimeBuilder` to carry JSON layers.
   - In `RuntimeBuilder` add `mcp_config_jsons: Vec<String>` and a builder method `mcp_config_jsons(...)`.
   - In `RuntimeBuilder::spawn_mcp`:
     ```rust
     let has_cli_mcp_override = !self.mcp_configs.is_empty() || !self.mcp_config_jsons.is_empty();
     let refs = if has_cli_mcp_override { self.mcp_configs } else { self.spec.mcp_config_refs.clone() };
     if !refs.is_empty() { builder = builder.from_mcp_config_refs(&refs).await?; }
     if !self.mcp_config_jsons.is_empty() { builder = builder.from_json_strs(&self.mcp_config_jsons).await?; }
     ```
   - For ACP, make startup-level JSON layers apply to every new/load session in addition to ACP `mcpServers` extras.

8. **Update TypeScript SDK types.**
   - Modify `packages/aether-sdk/src/types.ts`.
   - Add exported types mirroring the supported settings JSON:
     ```ts
     export type PromptEntry = string | { text: string };
     export interface AetherSettings {
       prompts?: PromptEntry[];
       mcpServers?: Array<string | { path: string; proxy?: boolean }>;
       agents?: AetherAgentSettings[];
     }
     export interface AetherAgentSettings {
       name: string;
       description: string;
       model: string;
       reasoningEffort?: ReasoningEffort;
       userInvocable?: boolean;
       agentInvocable?: boolean;
       prompts?: PromptEntry[];
       mcpServers?: Array<string | { path: string; proxy?: boolean }>;
       tools?: { allow?: string[]; deny?: string[] };
     }
     ```
   - Re-export these types from `packages/aether-sdk/src/index.ts`.

9. **Update TypeScript SDK process spawning.**
   - Modify `packages/aether-sdk/src/session.ts`:
     - Add `settings?: AetherSettings` and optionally `settingsFile?: string` to `AetherSessionOptions`.
     - Validate that `settings` and `settingsFile` are not both provided; throw `AetherSdkError` for conflicts.
     - When `settings` exists, push `--settings-json`, `JSON.stringify(settings)` into the `aether acp` args.
     - When `settingsFile` exists, push `--settings-file`, `settingsFile`.
   - Leave `externalMcpServers` and `tools` unchanged; they already avoid `mcp.json` via ACP.

10. **Update documentation.**
    - Update `packages/aether-cli/README.md` if it documents ACP/headless args.
    - Update `packages/aether-sdk/README.md` with an example:
      ```ts
      await using session = await AetherSession.start({
        cwd: "/tmp/workdir-without-aether",
        settings: {
          agents: [{
            name: "sdk-agent",
            description: "Agent supplied by the SDK host",
            model: "anthropic:claude-sonnet-4-5",
            userInvocable: true,
            prompts: [{ text: "You are running inside my host app." }],
            tools: { allow: ["custom__*"] },
          }],
        },
        agent: { agent: "sdk-agent" },
        tools: { custom: [myTool] },
      });
      ```
    - Document `--settings-json`, `--settings-file`, and `--mcp-config-json` in CLI help/README material.

## Testing Plan

### Unit tests required

- `packages/aether-project/src/settings.rs`
  - Existing string prompt entries still parse and resolve.
  - `{ "text": "..." }` prompt entries parse and become `Prompt::Text` in resolved `AgentSpec.prompts`.
  - Empty/whitespace inline prompt text is rejected with a specific `SettingsError` variant or clear `ParseError`/validation error.
  - `load_agent_catalog_from_source(..., AgentCatalogSource::Settings(settings))` works when no `.aether/settings.json` exists.
  - `AgentCatalogSource::Settings(...)` does not read or merge an existing `.aether/settings.json`.

- `packages/aether-cli/src/headless/mod.rs` / new `config_args.rs`
  - Clap accepts `--settings-json` and rejects combining it with `--settings-file`.
  - `resolve_spec` can select an inline settings agent from a temp cwd with no `.aether` directory.
  - `--mcp-config-json` participates in CLI override semantics.

- `packages/aether-core/src/mcp/mcp_builder.rs`
  - `from_json_strs` parses `servers` and `mcpServers` aliases.
  - Multiple JSON layers apply in order; last layer wins for duplicate names if merging is implemented at the raw-config layer.
  - In-memory MCP configs can be parsed after built-in factories are registered.

- `packages/aether-cli/src/acp/session_manager.rs`
  - `SessionManager` loads modes from inline settings source.
  - Default initial selection picks the first user-invocable inline agent.
  - Explicit `--agent` selection resolves against inline settings.

- `packages/aether-sdk/test/session.integration.test.ts`
  - Fake Aether receives `--settings-json` when `AetherSession.start({ settings })` is used.
  - `settings` + `settingsFile` conflict throws `AetherSdkError`.
  - Existing `tools` and `externalMcpServers` tests still pass.

### Integration tests needed

- Rust CLI-level test using temp cwd with no `.aether`:
  - Run or exercise `headless` resolution with `--settings-json` defining one agent with an inline prompt, then assert resolved agent name/model/prompt.
- SDK integration with fake ACP process:
  - Use a temp cwd with no `.aether` or `mcp.json`.
  - Start `AetherSession` with inline `settings`, selected `agent`, and a closure-backed tool.
  - Assert fake ACP logs include the expected CLI args and that the tool bridge still works.

### Edge cases to verify

- Malformed `--settings-json` returns a user-facing CLI error instead of panicking.
- `--settings-json` with `--agent missing` returns the existing unknown-agent error path.
- Inline settings with no user-invocable agent and no explicit model falls back exactly as current empty catalog behavior does, or fails if the settings validate no valid modes; choose and test the intended behavior.
- Inline text prompt plus MCP instructions are both included in the final system prompt.
- `--mcp-config-json` with invalid JSON returns a clear MCP error.
- `--mcp-config-json` does not accidentally trigger `cwd/mcp.json` fallback when present; CLI override should be explicit.

## Files to Modify/Create

| Path | Change | Status |
| --- | --- | --- |
| `packages/aether-project/src/settings.rs` | Add `AgentCatalogSource`, `load_agent_catalog_from_source`, `PromptEntry`, inline prompt validation/resolution, and tests. | Modified |
| `packages/aether-project/src/lib.rs` | Re-export new settings source and prompt-entry types/functions. | Modified |
| `packages/aether-project/src/error.rs` | Add a clear validation error for empty inline prompt text if not using an existing error. | Modified |
| `packages/aether-cli/src/config_args.rs` | New common Clap helper for `--settings-json` / `--settings-file`. | Added |
| `packages/aether-cli/src/lib.rs` | Export/use the new `config_args` module. | Modified |
| `packages/aether-cli/src/headless/mod.rs` | Add settings-source args and `--mcp-config-json`; resolve catalog from selected source; pass MCP JSON to runtime. | Modified |
| `packages/aether-cli/src/show_prompt/mod.rs` | Add settings-source args and `--mcp-config-json`. | Modified |
| `packages/aether-cli/src/show_prompt/run.rs` | Load catalog from selected source and pass MCP JSON to runtime. | Modified |
| `packages/aether-cli/src/acp/mod.rs` | Add settings-source args and optional startup `--mcp-config-json`; thread into `SessionManagerConfig`. | Modified |
| `packages/aether-cli/src/acp/session_manager.rs` | Store catalog source/startup MCP JSON layers and use them for new/load session catalog resolution. | Modified |
| `packages/aether-cli/src/acp/session.rs` | Accept startup MCP JSON layers and pass them to `RuntimeBuilder`. | Modified |
| `packages/aether-cli/src/runtime.rs` | Add `mcp_config_jsons` field/builder method and load JSON layers in `spawn_mcp`. | Modified |
| `packages/aether-core/src/mcp/mcp_builder.rs` | Add raw/JSON MCP config loading methods usable after factory registration. | Modified |
| `packages/aether-sdk/src/types.ts` | Add exported `AetherSettings`, `AetherAgentSettings`, `PromptEntry`, and tool filter types. | Modified |
| `packages/aether-sdk/src/session.ts` | Add `settings` / `settingsFile` options and serialize them to `aether acp` args. | Modified |
| `packages/aether-sdk/src/index.ts` | Re-export new SDK settings types. | Modified |
| `packages/aether-sdk/test/fakeAether.mjs` | Log process args so SDK tests can assert spawned CLI options. | Modified |
| `packages/aether-sdk/test/session.integration.test.ts` | Add tests for inline settings and no `.aether` cwd use case. | Modified |
| `packages/aether-sdk/README.md` | Document SDK inline settings and no-project-directory usage. | Modified |
| `packages/aether-cli/README.md` | Document new CLI settings/MCP JSON flags if this README contains command usage. | Modified |

## Additional Notes

- Before implementation, run `git diff` and keep the existing unstaged LSP/MCP changes intact. Do not reformat or rewrite those files beyond any necessary conflict-free imports if they are touched.
- Implement tests first for the new behavior, then update code until they pass.
- Prefer small commits/PR slices if this becomes large:
  1. `aether-project` settings-source + inline prompt support.
  2. CLI settings-source plumbing.
  3. MCP JSON config plumbing.
  4. TypeScript SDK types/spawn args/docs.
- After implementation, run:
  - `just fmt`
  - `just test`
  - `pnpm --filter @aether-agent/sdk typecheck`
  - `pnpm --filter @aether-agent/sdk test`

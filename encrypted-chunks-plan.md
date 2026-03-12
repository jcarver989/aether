# Encrypted Thought Chunks Implementation Plan

## Goal

Add support for model-specific encrypted reasoning state without breaking:

1. model switching mid-session
2. alloyed models that select a model per turn
3. session restore
4. context compaction

No backwards compatibility work is required.

---

## Final state

### Conversation state

Assistant turns store:

- visible assistant text
- visible reasoning summary text, if any
- optional encrypted reasoning content
- the exact `LlmModel` that produced that encrypted reasoning content
- tool calls

Encrypted reasoning content is assistant-turn state. It is not stored on tool calls.

### Request building

All outbound requests use a projected context for the selected `LlmModel`.

Projection rules:

- all portable messages remain unchanged
- assistant `summary_text` remains unchanged
- assistant `encrypted_content` is kept only when `encrypted_content.model == target_model`
- all other encrypted reasoning content is removed from the outbound request context

Provider implementations decide internally whether projected encrypted reasoning content is used by the target API.

### Restore and persistence

Session restore loads canonical `Context` directly from persisted state.

Event logs remain replay-only and are not used to reconstruct canonical context.

### Compaction

Compaction preserves portable text only.

Encrypted reasoning state from compacted turns is discarded.

### Auto-continue

Auto-continue preserves the in-progress assistant turn’s:

- visible text
- visible reasoning summary text
- encrypted reasoning content

---

## Data model changes

### `packages/llm/src/chat_message.rs`

Replace assistant reasoning text with structured reasoning metadata.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedReasoningContent {
    pub model: LlmModel,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AssistantReasoning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_text: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<EncryptedReasoningContent>,
}

pub enum ChatMessage {
    Assistant {
        content: String,
        reasoning: AssistantReasoning,
        timestamp: IsoString,
        tool_calls: Vec<ToolCallRequest>,
    },
    // ...
}
```

Update all call sites to use `reasoning.summary_text` instead of `reasoning_content`.

### `packages/llm/src/llm_response.rs`

Extend reasoning responses to carry encrypted reasoning payloads.

```rust
pub enum LlmResponse {
    Start { message_id: String },
    Text { chunk: String },
    Reasoning {
        model: LlmModel,
        summary_chunk: Option<String>,
        encrypted_content: Option<String>,
    },
    ToolRequestStart { id: String, name: String },
    ToolRequestArg { id: String, chunk: String },
    ToolRequestComplete { tool_call: ToolCallRequest },
    Done { stop_reason: Option<StopReason> },
    Error { message: String },
    Usage { input_tokens: u32, output_tokens: u32 },
}
```

`LlmResponse::Reasoning` must contain at least one of `summary_chunk` or `encrypted_content`.

---

## Shared context API changes

### `packages/llm/src/context.rs`

Add model projection and update assistant turn writes.

```rust
impl Context {
    pub fn project_for_model(&self, model: LlmModel) -> Context;

    pub fn estimated_token_count_for_model(&self, model: LlmModel) -> u32;

    pub fn push_assistant_turn(
        &mut self,
        content: &str,
        reasoning: AssistantReasoning,
        completed_tools: Vec<Result<ToolCallResult, ToolCallError>>,
    );
}

impl AssistantReasoning {
    pub fn from_parts(
        summary_text: &str,
        encrypted_content: Option<EncryptedReasoningContent>,
    ) -> Self;

    pub fn encrypted_content_for(&self, model: LlmModel) -> Option<&str>;

    pub fn projected_for(&self, model: LlmModel) -> Self;

    pub fn is_empty(&self) -> bool;
}
```

`project_for_model` keeps encrypted reasoning only for the selected model.

`estimated_token_count_for_model` must estimate tokens after model projection.

---

## Provider trait and request mapping

### `packages/llm/src/provider.rs`

Do not add any new capability API to `StreamingModelProvider`.

```rust
pub trait StreamingModelProvider: Send + Sync {
    fn stream_response(&self, context: &Context) -> LlmResponseStream;
    fn display_name(&self) -> String;
    fn context_window(&self) -> Option<u32>;
}
```

Provider implementations are responsible for:

- parsing encrypted reasoning content from responses when supported
- using projected encrypted reasoning content in requests when supported
- ignoring encrypted reasoning content when unsupported

### Provider request mapping

All request builders and mappers must consume projected context, not canonical context.

For assistant messages:

- map `reasoning.summary_text` where the target API supports visible reasoning text
- map `reasoning.encrypted_content` only when the projected content matches the active model and the target API supports encrypted reasoning state

---

## Agent changes

### `packages/aether-core/src/core/agent.rs`

Update iteration state to track visible and encrypted reasoning separately.

```rust
struct IterationState {
    current_message_id: Option<String>,
    message_content: String,
    reasoning_summary_text: String,
    encrypted_reasoning: Option<EncryptedReasoningContent>,
    pending_tool_ids: HashSet<String>,
    completed_tool_calls: Vec<Result<ToolCallResult, ToolCallError>>,
    llm_done: bool,
    stop_reason: Option<StopReason>,
    cancelled: bool,
}
```

Required behavior:

- `AgentMessage::Thought` remains driven only by visible reasoning summary text
- encrypted reasoning state is never emitted to UI-facing messages
- `LlmResponse::Reasoning.summary_chunk` appends to `reasoning_summary_text`
- `LlmResponse::Reasoning.encrypted_content` replaces `encrypted_reasoning` with `EncryptedReasoningContent { model, content }`
- when an assistant turn is committed, both tool calls and encrypted reasoning content are stored on the same assistant turn if both were produced

Before every request:

- determine the selected `LlmModel`
- project context with `context.project_for_model(model)`
- send the projected context to the provider

Use `estimated_token_count_for_model(model)` for request-time token estimation and compaction thresholds.

---

## Alloyed model changes

### `packages/llm/src/alloyed.rs`

`AlloyedModelProvider` must:

1. select the next `LlmModel`
2. project the context for that model
3. pass the projected context to the selected inner provider

Suggested helper:

```rust
impl AlloyedModelProvider {
    fn select_next_provider(&self) -> Option<(LlmModel, &dyn StreamingModelProvider)>;
}
```

---

## Codex changes

### `packages/llm/src/providers/codex/streaming.rs`

Parse encrypted reasoning stream events and emit:

```rust
LlmResponse::Reasoning {
    model: active_model,
    summary_chunk: ..., 
    encrypted_content: ..., 
}
```

### `packages/llm/src/providers/codex/mappers.rs`

Map assistant turns from projected context into Codex request items.

Required behavior:

- carry forward compatible encrypted reasoning content from prior assistant turns
- preserve coexistence of encrypted reasoning content and tool/function call items on the same assistant turn

Suggested signature:

```rust
pub fn map_messages(messages: &[ChatMessage], model: LlmModel) -> (Option<String>, Vec<InputItem>)
```

---

## Session persistence changes

### `packages/aether-cli/src/acp/session_store.rs`

Persist canonical state separately from replay events.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSessionState {
    pub meta: SessionMeta,
    pub context: Context,
}

impl SessionStore {
    pub fn save_state(&self, session_id: &str, state: &PersistedSessionState) -> io::Result<()>;
    pub fn load_state(&self, session_id: &str) -> Option<PersistedSessionState>;
    pub fn append_event(&self, session_id: &str, event: &SessionEvent) -> io::Result<()>;
    pub fn load_events(&self, session_id: &str) -> Option<Vec<SessionEvent>>;
}
```

Filesystem layout:

- `~/.aether/sessions/{session_id}.state.json`
- `~/.aether/sessions/{session_id}.events.jsonl`

### `packages/aether-cli/src/acp/session_manager.rs`

Restore flow:

- load `.state.json`
- seed the session from `state.context`
- load `.events.jsonl` for replay only

Stop using `Context::from_events()` for session restore.

### `packages/aether-cli/src/acp/session.rs`

Update `Session::new(...)` to accept restored canonical context.

```rust
pub async fn new(
    llm: impl StreamingModelProvider + 'static,
    mcp_config_path: Option<PathBuf>,
    cwd: PathBuf,
    extra_mcp_servers: Vec<McpServerConfig>,
    prompt_patterns: Vec<String>,
    restored_context: Option<Context>,
) -> Result<Self, Box<dyn std::error::Error>>
```

Persist canonical state after every context mutation, including:

- user message added
- assistant turn completed
- context cleared
- compaction completed

---

## Compaction and auto-continue

### Compaction

### `packages/aether-core/src/context/compaction.rs`

When compacting turns into a summary:

- preserve system messages
- preserve summary text
- discard encrypted reasoning state from compacted turns
- do not attach encrypted reasoning state to `Summary` messages

### Auto-continue

### `packages/aether-core/src/core/agent.rs`

When continuing after a partial assistant response, inject an assistant turn that preserves:

- accumulated assistant text
- accumulated visible reasoning summary text
- accumulated encrypted reasoning content

```rust
ChatMessage::Assistant {
    content: previous_response.to_string(),
    reasoning: AssistantReasoning {
        summary_text: previous_reasoning_summary,
        encrypted_content: previous_encrypted_reasoning,
    },
    timestamp: IsoString::now(),
    tool_calls: Vec::new(),
}
```

---

## Implementation sequence

1. Update `ChatMessage`, `AssistantReasoning`, `EncryptedReasoningContent`, `LlmResponse`, and `Context` APIs in `packages/llm`.
2. Update agent accumulation and request entrypoints to project by `LlmModel` and store encrypted reasoning state.
3. Update alloyed model dispatch to project context for the selected model before streaming.
4. Update Codex streaming and request mapping to parse and replay encrypted reasoning content.
5. Update other providers/mappers to ignore or use projected encrypted reasoning content as appropriate.
6. Redesign session persistence to store canonical `Context` separately from replay events.
7. Update compaction and auto-continue to preserve or drop encrypted reasoning state according to the rules above.
8. Add and run tests.

---

## Tests

### `llm` tests

Add tests for:

- `project_for_model` preserves encrypted reasoning content for the matching model
- `project_for_model` removes encrypted reasoning content for non-matching models
- summary text remains portable after projection
- assistant tool calls remain intact after projection
- `estimated_token_count_for_model` reflects projected context

### `aether-core` tests

Add tests for:

- encrypted reasoning emitted by a model is stored in canonical context
- a turn that emits encrypted reasoning content and tool calls is persisted as one assistant turn containing both
- switching models removes incompatible encrypted reasoning content from outbound requests
- switching back to the original model restores compatible encrypted reasoning content
- auto-continue preserves encrypted reasoning content
- compaction drops encrypted reasoning state from compacted turns

### `llm` alloy tests

Add tests for:

- model A turns include model-A encrypted reasoning content
- model B turns exclude model-A encrypted reasoning content
- later model A turns include model-A encrypted reasoning content again

### `aether-cli` persistence tests

Add tests for:

- canonical context save/load round-trips encrypted reasoning content
- replay logs still replay visible transcript correctly
- session restore uses canonical context and does not depend on `Context::from_events`

### Codex tests

Add tests for:

- encrypted reasoning stream events become `LlmResponse::Reasoning`
- Codex requests include encrypted reasoning content from compatible prior assistant turns
- non-matching model projections exclude that encrypted reasoning content

---

## Acceptance criteria

The implementation is complete when all of the following are true:

- assistant turns persist visible reasoning summaries and optional encrypted reasoning content tagged with `LlmModel`
- outbound requests always use `Context::project_for_model(target_model)`
- encrypted reasoning content is replayed only to the same model that produced it
- encrypted reasoning content can coexist with tool calls on the same assistant turn
- session restore loads canonical context directly from persisted state
- compaction drops encrypted reasoning state from compacted turns
- auto-continue preserves encrypted reasoning state for the in-progress turn
- Codex parses and replays encrypted reasoning content correctly

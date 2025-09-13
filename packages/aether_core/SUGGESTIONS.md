# Suggestions for aether_core

## 1. Architecture & Design
- **Separation of concerns** – The `Agent` struct mixes LLM interaction, MCP management, and message state. Consider extracting a dedicated *conversation* or *state machine* component that owns the chat history and tool‑call logic. This would make the `Agent` thin and easier to test.
- **Tool namespace** – The current `namespaced_tool_name` format (`server__tool`) is fragile if a tool name contains `__`. Use a struct or a tuple `(String, String)` and expose a `ToolId` type that implements `Display`/`FromStr`.
- **Built‑in MCPs** – `BuiltinMcp` is an enum but only has one variant. If more built‑ins are added, the `match` in `AgentBuilder::build` will grow. Store a map of `BuiltinMcp` to a factory closure.

## 2. Error handling
- **Consistent error types** – `Agent::run_agent_loop` returns `AgentMessage::Error` with a string. Wrap these in a custom error enum (`AgentError`) and expose it via `Result<impl Stream<Item = AgentMessage>, AgentError>`. This gives callers richer context.
- **Propagation of LLM errors** – The LLM stream can produce `Err(e)`. Currently we yield an `AgentMessage::Error` but the stream type is still `impl Stream<Item = AgentMessage>`. Consider returning `Result<impl Stream<Item = AgentMessage>, AgentError>` so the caller can decide to abort.
- **Tool execution errors** – `McpManager::execute_tool` returns a `Result<Value>`. The error message is a string; use `thiserror::Error` to provide structured errors (e.g., `ToolNotFound`, `ExecutionFailed`).

## 3. Concurrency & Cancellation
- **Cancellation token usage** – The spawned task in `AgentBuilder::spawn` never consumes the returned `CancellationToken`. Pass the token to the task or expose a `cancel()` method on `Agent` that signals the token and closes the channel.
- **Back‑pressure** – The user channel has a capacity of 100. If the LLM is slow, the channel may fill and block the user. Consider using a bounded channel with a timeout or a `select!` that drops or buffers messages.
- **Graceful shutdown** – `McpManager::shutdown` drops the client before waiting for the server task. This is fine, but the server task may still be running. Add a `shutdown_timeout` constant and log if the task does not finish.

## 4. Testing & CI
- **Test coverage** – Most tests exercise the public API, but there are no tests for error paths (e.g., tool not found, LLM error). Add tests that simulate a failing tool and verify that `AgentMessage::Error` is produced.
- **Property‑based tests** – Use `proptest` to generate random chat histories and tool calls to ensure the agent loop does not dead‑lock.
- **CI pipeline** – Add a `cargo clippy --all-targets --all-features` step and enforce `#![deny(clippy::all)]` in `lib.rs`.

## 5. Documentation & Code Style
- **Doc comments** – Add `///` comments to public structs and functions (`Agent`, `AgentBuilder`, `McpManager`). Explain the message flow and the meaning of `message_id`.
- **Naming** – `McpServerConfig` variants use `Http` and `Stdio`. Rename to `Http` → `HttpServer` and `Stdio` → `StdioServer` for clarity.
- **Constants** – `MAX_ITERATIONS` is magic. Define `const MAX_AGENT_ITERATIONS: usize = 10_000;` and document the rationale.
- **Error messages** – Use `format!` with `?` to propagate errors instead of `Report::msg`. This keeps stack traces.

## 6. Dependencies & Build
- **async-openai** – The crate is used only for the `ModelProvider` trait. If the project grows, consider abstracting the LLM behind a trait object to allow swapping implementations without pulling the heavy dependency.
- **rmcp** – The `rmcp` crate brings in many features. Disable unused features (`client`, `server`, `macros`, `schemars`, `transport-streamable-http-client-reqwest`) via `default-features = false` and enable only what is needed.
- **`cargo audit`** – Run regularly to catch any new vulnerabilities.

## 7. Potential Bugs & Edge Cases
- **Tool name collision** – Two servers may expose a tool with the same name. The current namespacing solves this, but the `split("__").nth(1)` logic will break if a tool name contains `__`. Use `rsplitn(2, "__")`.
- **Partial tool arguments** – `ToolRequestArg` events may arrive in fragments. The current implementation concatenates them but does not handle malformed JSON until `ToolRequestComplete`. Consider validating after each fragment.
- **Infinite loops** – If the LLM keeps requesting tools, the agent will loop until `MAX_ITERATIONS`. Add a configurable timeout per iteration.

## 8. Future Enhancements
- **Streaming LLM responses** – Expose a `Stream` of `AgentMessage` directly from `Agent::send` without the intermediate `run_agent_loop` function.
- **Tool caching** – Cache tool definitions per server to avoid repeated `list_tools` calls.
- **Dynamic server discovery** – Allow the agent to discover MCP servers over the network (e.g., via mDNS) instead of manual configuration.
- **Metrics** – Integrate `tracing` spans for each tool call and LLM request to aid observability.

---

**Next steps** – Prioritize the error‑handling refactor and the cancellation token integration. Once those are in place, add the missing tests for error paths.

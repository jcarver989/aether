# Suggestions for aether_core

## 1. Documentation & API surface
- Add `///` doc comments to all public items (`pub mod`, `pub struct`, `pub enum`, `pub fn`).  The current code is functional but lacks documentation, which makes it hard for new contributors to understand the intent.
- For the `AgentBuilder` API, document the semantics of `system`, `mcp`, `coding_tools`, and `build`.  Mention that `build` consumes the builder and returns a fully configured `Agent`.
- Document the `McpManager` methods, especially the difference between `with_http_mcp`, `with_stdio_mcp`, and `with_in_memory_mcp`.
- Add module level documentation for `llm`, `mcp`, `testing`, and `types`.

## 2. Error handling & Result propagation
- Mark all public functions that return `Result` with `#[must_use]` to avoid silent failures.
- Prefer `anyhow::Result` for library code that is not part of a CLI.  The current code uses `color_eyre::Result`, which is fine for a CLI but may be too heavy for a library.
- In `Agent::run_agent_loop`, the `match` on `LlmResponse` could be refactored into a separate method to reduce nesting and improve readability.
- In `McpManager::execute_tool`, the error messages could be more descriptive by including the tool name and server name in the error context.

## 3. Concurrency & async patterns
- The `Agent::run_agent_loop` uses a `stream!` macro that captures `self` by value.  Consider using `Arc<Mutex<>>` for shared mutable state if the agent will be used from multiple tasks.
- The `McpManager::shutdown` method currently drops the client before waiting for the server task.  This is fine, but adding a timeout constant (e.g. `const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);`) would make the timeout configurable.
- In `McpManager::with_in_memory_mcp`, the server task is spawned with `tokio::spawn`.  Capture the task handle in a `JoinHandle<()>` and store it in `server_task`.  The current implementation already does this, but adding a `#[allow(clippy::unused_async)]` attribute on the closure can silence a warning.

## 4. Testing & test coverage
- Add integration tests for the `Agent` that exercise the full request/response cycle with a mock LLM provider.  The current `testing/full_integration.rs` covers the in‑memory MCP, but a test that uses the `LocalModelProvider` would be valuable.
- Add property‑based tests (e.g. with `proptest`) for the `IsoString::now` method to ensure it always returns a valid RFC‑3339 string.
- Add unit tests for `McpManager::discover_tools` that verify the tool names are correctly namespaced.

## 5. Code style & ergonomics
- Use `#[derive(Debug, Clone, PartialEq, Eq, Hash)]` consistently for all structs that are used as keys in `HashMap`.  The `Tool` struct already implements `Clone`, but adding `Hash` would allow it to be used as a key if needed.
- Replace the manual `for (tool_call, result_str) in completed_tool_calls` loop with an iterator that collects the results into a vector and then pushes them in a single call.  This reduces the number of `push` operations.
- In `Agent::run_agent_loop`, the `has_tool_calls` flag is set to `true` only when a `ToolRequestComplete` event is received.  Consider moving the flag update to the `ToolRequestComplete` match arm to avoid accidental false positives.
- Use `std::io::Write::flush` only once per loop iteration instead of inside the `if is_complete` block.  This reduces the number of flushes.

## 6. Performance & memory
- The `Agent::run_agent_loop` clones the entire message history on every iteration.  For long conversations this can become expensive.  Consider storing the history in a `VecDeque` and only cloning the tail that the LLM needs.
- The `McpManager::discover_tools` clears the entire tool cache on each call.  If the set of tools rarely changes, consider caching the result and invalidating only when a server is added or removed.

## 7. Security & safety
- The `McpManager::with_stdio_mcp` currently returns an error.  If this feature is not supported, document it in the public API and consider removing the method to avoid confusion.
- The `Agent::send` method returns a `CancellationToken`.  Document the semantics of this token and how it should be used by callers.
- Ensure that all external crates are up to date and free of known vulnerabilities.  Run `cargo audit` regularly.

## 8. Dependency hygiene
- The project currently depends on `color_eyre`, `rmcp`, `serde_json`, `tokio`, `async-stream`, `futures`, `specta`, `chrono`, and `clap`.  Verify that each crate is necessary and that no transitive dependencies introduce security issues.
- Consider replacing `color_eyre` with `anyhow` for library code to reduce binary size.

---

These suggestions aim to improve the maintainability, performance, and safety of the `aether_core` crate while keeping the code idiomatic and well‑documented.

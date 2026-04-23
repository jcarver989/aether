# Refactor Plan

Review target: current branch staged changes, including ACP 0.11 migration, ACP actor removal, extension codec work, Wisp updates, and Codex streaming changes.

Validation observed during review:

- `git diff --cached --check` passed.
- LSP workspace diagnostics showed no errors/warnings beyond existing hints.
- No unstaged changes were present at review time.
- Focused tests exposed regressions:
  - `cargo test -p aether-agent-cli --lib -- --nocapture` failed 3 extension-method-name tests.
  - `cargo test -p aether-wisp settings_menu_tests -- --nocapture` failed the auth-methods-updated notification test.

## P0: Make extension method naming impossible to misuse

The branch introduces a logical-vs-wire extension method distinction:

- Logical application name: `aether/mcp`
- JSON-RPC wire name: `_aether/mcp`

`packages/acp-utils/src/ext_codec.rs` is the right centralization point, but production code and tests still compare raw method strings directly. That has already caused failures.

Failing examples:

- `packages/aether-cli/src/acp/mappers.rs:466`
- `packages/aether-cli/src/acp/mappers.rs:644`
- `packages/aether-cli/src/acp/relay.rs:676`
- `packages/wisp/src/components/app/mod.rs:403`

Recommended refactor:

1. Treat `ext_codec` as the only place that understands logical-vs-wire method formatting.
2. Add helpers such as:
   - `logical_notification_method(&ExtNotification) -> &str`
   - `notification_method_matches(&ExtNotification, expected: &str) -> bool`
   - `assert_wire_method(...)` for tests, if useful
3. Update `App::on_ext_notification` to normalize once before matching.
4. Update tests to assert intentionally:
   - Encoded outbound notifications use wire names.
   - ACP-dispatched inbound notifications use logical names.
   - App handlers accept either form if tests construct notifications directly.

Preferred end state: no code outside `ext_codec` or `notifications` calls `notification.method.as_ref()` for business routing.

## P0/P1: Make relay cancellation cooperative during active prompts

The new `RelayHandle` owns a `CancellationToken`, which is good, but cancellation is only checked in the idle outer loop:

- `packages/aether-cli/src/acp/relay.rs:122-145`

Once a relay enters active prompt handling, shutdown cancellation is no longer observed:

- `packages/aether-cli/src/acp/relay.rs:179-267`

`SessionManager::shutdown_all_sessions()` cancels relays and then awaits them:

- `packages/aether-cli/src/acp/session_manager.rs:216-225`

That can still hang if a relay is inside an active prompt, MCP event, or elicitation flow.

Recommended refactor:

1. Thread `CancellationToken` into `PromptContext`.
2. Add a cancellation branch inside `run_turn_loop`.
3. On cancellation, send `UserMessage::Cancel` to the agent and return a cancellation result that unblocks the pending ACP request.
4. Replace separate `cancel()` and `join()` call-site choreography with a single `RelayHandle::stop(self).await` method where possible.

Preferred end state: `shutdown_all_sessions()` cannot hang on an active turn unless the underlying agent task itself is uninterruptible, and that case is explicit in the type/API.

## P1: Simplify `run_acp` lifecycle and connection ownership

`run_acp` is much simpler after actor removal, but the lifecycle is still implicit:

- `packages/aether-cli/src/acp/mod.rs:69-85` attaches a connection and then awaits `std::future::pending::<()>()`.
- `packages/aether-cli/src/acp/mod.rs:72-75` contains an `_manager = manager.clone()` comment/capture that is misleading; because `_manager` is not referenced inside the closure body, it likely does not provide the lifetime guarantee the comment claims.
- `packages/aether-cli/src/acp/mod.rs:107-118` classifies clean disconnects by matching stringified IO error messages in `err.data`.

Recommended refactor:

1. Introduce a small ACP server runtime/lease abstraction:
   - `AcpServerRuntime { manager, connection }`
   - `ConnectionLease` returned by `connection.attach(cx)`
2. Make detach tied to the lease rather than a global unconditional call.
3. Replace `pending()` with an explicit shutdown signal or a well-named runtime foreground future.
4. Keep the SDK transport-close workaround in one small tested adapter.

Preferred end state: connection attach/detach ownership is structural, and future readers do not need to understand `connect_with` foreground semantics to reason about shutdown.

## P1: DRY ACP handler registration — evaluated, kept as-is

Outcome: repetition is retained. `on_receive_request` is typed as
`AsyncFnMut(Req, Responder<Resp>, Conn) -> Result<T, Error>`, paired with a
companion dispatch value produced by the `acp::on_receive_request!()` macro at
the call site. `SessionManager`'s async methods are `for<'a> fn(&'a SessionManager, Req) -> impl Future<..>`.
A generic helper would need higher-ranked trait bounds for a `Future` type that
borrows `&'a SessionManager`, which is a known-awkward Rust pattern; it cost
more clarity than the repetition saves. Per user preference, no `macro_rules!`
was introduced. A short explanatory comment at the top of `handlers.rs` records
the decision.

## P1/P2: Split `SessionManager` responsibilities

`packages/aether-cli/src/acp/session_manager.rs` is now responsible for:

- ACP request business logic
- session registry
- relay lifecycle
- auth orchestration
- config option state
- model/mode validation
- session creation/loading/replay
- outbound notification fanout

Recommended larger refactor:

1. `SessionRegistry`
   - Owns `HashMap<String, SessionState>`.
   - Owns relay lifecycle and shutdown.
2. `SessionFactory`
   - Creates and loads sessions.
   - Resolves mode catalog and session metadata.
3. `ConfigService`
   - Applies config mutations.
   - Builds model/mode/reasoning config options.
4. `AuthCoordinator`
   - Performs provider auth.
   - Emits auth-method/config refresh notifications.
5. `SessionManager`
   - Becomes a thin facade called by ACP handlers.

Preferred end state: domain behavior can be tested without ACP transport wiring, and `handlers.rs` remains a pure adapter layer.

## P2: Strengthen `AcpConnectionHandle` lifecycle invariants

`packages/acp-utils/src/server/connection_handle.rs` is a good simplification over the old actor, but its lifecycle invariants are weak:

- `attach()` silently overwrites an existing connection.
- `detach()` clears whichever connection is currently stored.
- If reconnects are ever introduced, an old owner could detach a newer connection.

Recommended refactor:

- If the ACP server is truly one-shot, use `OnceCell<ConnectionTo<Client>>` and remove reattach complexity.
- If reconnects are valid, make `attach()` return a generation-based `ConnectionLease`; only the matching lease can detach.

Preferred end state: stale connection owners cannot affect newer connections by construction.

## P2: Keep server errors typed

`AcpConnectionHandle` currently maps protocol errors into strings in several places:

- `packages/acp-utils/src/server/connection_handle.rs:47`
- `packages/acp-utils/src/server/connection_handle.rs:54`
- `packages/acp-utils/src/server/connection_handle.rs:63`
- `packages/acp-utils/src/server/connection_handle.rs:69-70`

Recommended refactor:

```rust
enum AcpServerError {
    ConnectionUnavailable,
    Protocol {
        operation: &'static str,
        source: agent_client_protocol::Error,
    },
}
```

Add a helper for mapping operation-specific protocol errors.

Preferred end state: callers can pattern-match specific server failure modes, and source errors are preserved.

## P2: Add fake-friendly test seams after actor removal

Deleting the actor simplified production code, but some tests lost the ability to assert outbound behavior. Several tests became pure mapping tests instead of exercising relay/session interactions.

Recommended refactor:

1. Add a small private trait for outbound ACP operations used by relay/session replay:
   - `send_session_notification`
   - `send_ext_notification`
   - `ext_method`
   - `request_permission`, if needed
2. Implement it for `AcpConnectionHandle`.
3. Use an in-memory fake in relay/session-manager tests.

Good fake-backed test targets:

- URL elicitation notification forwarding
- session replay outbound notifications
- ext-method elicitation response handling
- relay shutdown during active prompts
- connection unavailable behavior

Preferred end state: relay behavior is tested against stateful fakes rather than only pure mappers or concrete ACP transports.

## P3: Clean up staged planning doc

`acp-followups-plan.md` appears to be an implementation scratch plan rather than durable project documentation. It is also stale/inconsistent with the current implementation:

- It both recommends and rejects an `acp_utils::protocol` facade.
- It says to use `Arc<RwLock<Option<_>>>`, while implementation uses `Mutex`.
- It says all ACP-related tests should pass, but they currently do not.

Recommended action:

- Move this content to the PR description or issue, or
- Rewrite it as a short ADR that documents the final architecture and tradeoffs.

Preferred end state: committed docs describe the architecture as implemented, not the intermediate plan.

## P3: Remove now-unused dependencies

After removing ACP trait implementations, some dependencies may now be unused:

- `packages/acp-utils/Cargo.toml` still has `async-trait`; no `async_trait` use was found under `packages/acp-utils`.
- `packages/aether-cli/Cargo.toml` still has `async-trait`; no local use was found under `packages/aether-cli`.

Recommended action:

- Confirm with `cargo machete` or an equivalent dependency check.
- Remove unused dependencies if confirmed.

## Positive architecture to preserve

Keep these changes:

- Removing `AcpActor` / `AcpActorHandle` is a strong simplification.
- `ext_codec.rs` is the right place to centralize extension JSON encoding/decoding.
- `AcpRunOutcome` / `AcpRunError` is better than returning raw ACP errors to `main.rs`.
- Splitting client startup errors into invalid command, connect failure, and protocol failure improves user-facing diagnostics.
- Moving ACP request registration into `handlers.rs` is the right adapter-layer separation.

## Suggested execution order

1. Fix extension method normalization and update failing tests.
2. Make relay cancellation cooperative inside active prompts.
3. Wrap `run_acp` connection lifecycle in a lease/runtime abstraction.
4. DRY `handlers.rs` with a small forwarding macro.
5. Split `SessionManager` into registry/factory/config/auth components.
6. Strengthen `AcpConnectionHandle` lifecycle and error types.
7. Add fake-backed relay/session tests.
8. Clean docs and unused dependencies.

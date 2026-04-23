AcpConnection trait also looks like something that would already be in the acp crate, or at least
  something similar to it ?

  Searched for 5 patterns, read 1 file (ctrl+o to expand)

● Good question. Two things are going on:

  What's in the ACP crate: ConnectionTo<Counterpart> (jsonrpc.rs:1433) is the concrete sender and already
  has generic send_notification<N: JsonRpcNotification> / send_request<R: JsonRpcRequest> methods — so
  AcpConnectionHandle's send_notification<N> / send_request<R> wrappers are just error-type translators
  over that.

  Why the AcpConnection trait exists: it's used exclusively as a trait object — Arc<dyn AcpConnection> /
  &dyn AcpConnection — threaded through SessionManager, Relay, replay_to_client. Prod wires
  AcpConnectionHandle; tests wire FakeAcpConnection from testing.rs. ConnectionTo<Client> can't fill that
  role directly because you can't construct one without a live transport, and generic methods
  (send_notification<N>) aren't dyn-safe.

  The price of dyn-safety: enumerating every payload type as its own method (send_context_usage,
  send_auth_methods_updated, …) looks repetitive, but it's what preserves the compile-time type safety the
   notifications refactor was protecting. The alternative dyn-safe shapes all regress:

  ┌────────────────────────────────────────────────────┬──────────────────────────────────────────────┐
  │                       Shape                        │                   Problem                    │
  ├────────────────────────────────────────────────────┼──────────────────────────────────────────────┤
  │ fn send_notification_dyn(&self, method: &str,      │ Reintroduces the magic-string API we just    │
  │ params: RawValue)                                  │ killed.                                      │
  ├────────────────────────────────────────────────────┼──────────────────────────────────────────────┤
  │ enum Connection { Real(Handle), Fake(Fake) }       │ Works, but viralizes Fake into the prod      │
  │                                                    │ crate.                                       │
  ├────────────────────────────────────────────────────┼──────────────────────────────────────────────┤
  │ <C: Connection> generic threaded through           │ Spreads a generic parameter across Relay,    │
  │                                                    │ SessionManager, etc.                         │
  └────────────────────────────────────────────────────┴──────────────────────────────────────────────┘

  So the trait isn't redundant with ConnectionTo — it's the seam that lets us keep typed sends AND have
  in-memory test fakes. That said, the doc comment on the trait should probably call this out explicitly
  ("this is a dyn-safe projection of ConnectionTo's generic send API — new wire methods go here"). Want me
   to tighten that comment, or leave commit 2 as-is and move on?

✻ Crunched for 1m 5s

❯ This seems odd though because ConnectionTo is already generic yes? So wouldn't the right thing to do be
   have a fake that ConnectionTo can use?

  Searched for 3 patterns, read 2 files, listed 4 directories (ctrl+o to expand)

● Good call — you're right that this is the more honest way to frame it. Let me walk through the actual
  tradeoff:

  What ConnectionTo<T> is generic over: the peer role (Client, Agent, custom), not the transport. You
  can't swap transports on a constructed ConnectionTo. But the ACP builder takes impl ConnectTo<Host>
  (jsonrpc.rs:1099), and the sanctioned way to test against it is exactly what the ACP test suite does in
  tests/jsonrpc_hello.rs: a tokio::io::duplex(1024) pair wrapped in ByteStreams, with a real
  ConnectionTo<Client> on one end and a "test peer" consumer on the other.

  So the alternative design is:
  - Drop the AcpConnection trait entirely.
  - SessionManager / Relay / replay_to_client take Arc<AcpConnectionHandle> directly (we still keep that
  thin wrapper for error-type translation + the one-shot OnceLock invariant).
  - Tests build a ConnectionTo<Client> over an in-memory duplex, wrap in AcpConnectionHandle, run a peer
  task that receives SessionNotification / typed ext notifications, and assert on decoded values.

  Honest tradeoff:

  ┌────────────────────┬───────────────────────────────────────────┬──────────────────────────────────┐
  │                    │       Current: dyn AcpConnection +        │ Real ConnectionTo over in-memory │
  │                    │             FakeAcpConnection             │              duplex              │
  ├────────────────────┼───────────────────────────────────────────┼──────────────────────────────────┤
  │ Test coverage      │ Stops before serialize/dispatch           │ Exercises the real wire path     │
  ├────────────────────┼───────────────────────────────────────────┼──────────────────────────────────┤
  │ New wire method    │ Update typed struct + trait + both impls  │ Update typed struct only         │
  ├────────────────────┼───────────────────────────────────────────┼──────────────────────────────────┤
  │ Dyn-safety         │ Forces per-type method enumeration        │ N/A — one generic API            │
  │ constraint         │                                           │                                  │
  ├────────────────────┼───────────────────────────────────────────┼──────────────────────────────────┤
  │ Test setup cost    │ Zero (just FakeAcpConnection::new())      │ LocalSet + duplex + peer task    │
  ├────────────────────┼───────────────────────────────────────────┼──────────────────────────────────┤
  │ Assertion style    │ Sync read of Mutex<Vec<T>>                │ Async receive + deserialize      │
  └────────────────────┴───────────────────────────────────────────┴──────────────────────────────────┘

  The per-method enumeration in AcpConnection exists only because send_notification<N:
  JsonRpcNotification> isn't dyn-safe. If you drop dyn-ness, the whole trait dissolves.

  The refactor-plan.md recommended the trait approach (P2 item), but your instinct is better: the fake
  saves per-test setup cost but also skips the serialize/dispatch layer — which is exactly where subtle
  wire-format bugs live (and which the P0 item about extension-method naming just bit us on). The list of
  targets the plan wanted fake coverage for — URL elicitation forwarding, session replay, ext-method
  elicitation response handling — all involve round-tripping JSON, so they'd be strictly better served by
  the real-transport harness.

  Two options for this branch:

  1. Keep the trait, note the debt: leave commit 2 as-is; add a follow-up to replace FakeAcpConnection
  with a duplex-based harness later. Low-risk for the review-and-land goal.
  2. Rip out the trait now: delete connection.rs (the trait), rename connection_handle.rs back to
  something like just connection.rs, thread Arc<AcpConnectionHandle> through SessionManager/Relay, and
  rewrite the handful of tests in relay.rs and mappers.rs that construct FakeAcpConnection to use a duplex
   peer. Bigger churn but arguably correct.

  Which direction do you want to go?

✻ Sautéed for 1m 33s

❯ /context
  ⎿  Context Usage
     ⛁ ⛁ ⛁ ⛁ ⛀ ⛀ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁ ⛁   Opus 4.7 (1M context)
     ⛁ ⛁ ⛁ ⛁ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶   claude-opus-4-7[1m]
     ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶   112.4k/1m tokens (11%)
     ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶
     ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶   Estimated usage by category
     ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶   ⛁ System prompt: 9.6k tokens (1.0%)
     ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶   ⛁ System tools: 11k tokens (1.1%)
     ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶   ⛁ Memory files: 1k tokens (0.1%)
     ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶   ⛁ Skills: 1.1k tokens (0.1%)
     ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶ ⛶   ⛁ Messages: 92.4k tokens (9.2%)
                                               ⛶ Free space: 884.8k (88.5%)

     MCP tools · /mcp (loaded on-demand)

     Available
     ├ mcp__claude_ai_Gmail__authenticate
     ├ mcp__claude_ai_Gmail__complete_authentication
     ├ mcp__claude_ai_Google_Calendar__authenticate
     ├ mcp__claude_ai_Google_Calendar__complete_authentication
     ├ mcp__claude_ai_Google_Drive__authenticate
     └ mcp__claude_ai_Google_Drive__complete_authentication

     Memory files · /memory
     ├ CLAUDE.md: 19 tokens
     ├ AGENTS.md: 788 tokens
     └ ~/.claude/projects/-home-josh-code-aether/memory/MEMORY.md: 208 tokens

     Skills · /skills

     User
     ├ plannotator-compound: 140 tokens
     ├ plannotator-review: 20 tokens
     ├ plannotator-annotate: 18 tokens
     └ plannotator-last: 15 tokens

❯ what do we need dyn for anyways?

● Honest answer: only to support FakeAcpConnection.

  Concretely: SessionManager.connection: Arc<dyn AcpConnection> exists because two types have to fit that
  slot — AcpConnectionHandle in production and FakeAcpConnection in tests. If there were only one impl,
  there'd be no need for a trait at all, let alone dyn.

  Given two impls, you've got three ways to get polymorphism:

  1. Generic (SessionManager<C: AcpConnection>, Relay<C>, replay_to_client<C>(...)) — viralizes a type
  parameter through every call site that touches the connection, for zero runtime benefit.
  2. Arc<dyn AcpConnection> (current) — hides the generic but pays for it with dyn-safety: no
  send_notification<N> method, so every payload type is enumerated by hand.
  3. Enum wrapper — forces the test-only variant into the production crate (or #[cfg(test)] on production
  fields, which is explicitly off-limits per your prior feedback).

  All three only exist to support the fake. The fourth option — use a real AcpConnectionHandle in tests
  too, built over an in-memory duplex pipe — collapses to a single concrete type everywhere: no trait, no
  dyn, no generic, no fake. That's the cleanest resolution of the previous thread and this one together.

  So the real question isn't "why dyn" but "do we want the fake at all?" If we kill the fake, the trait
  and the dyn both go away, and SessionManager/Relay just hold Arc<AcpConnectionHandle>. Tests pay a
  one-time harness cost (~30 lines of duplex-peer setup) to get serialize/dispatch coverage that the fake
  can't provide.

  Want me to actually do this? The change is scoped:

  1. Delete packages/acp-utils/src/server/connection.rs (the trait).
  2. Remove BoxFuture imports from wherever (already done once; will need a similar follow-up in
  testing.rs when we remove it).
  3. Change Arc<dyn AcpConnection> → Arc<AcpConnectionHandle> in session_manager.rs and relay.rs, and &dyn
   AcpConnection → &AcpConnectionHandle in mappers.rs::replay_to_client + the relay helpers.
  4. Replace FakeAcpConnection in testing.rs with a duplex-based peer helper: fn test_connection() ->
  (AcpConnectionHandle, TestPeer) where TestPeer exposes async receive methods for decoded typed messages.
  5. Rewrite the ~4 test sites in relay.rs and the one in mappers.rs to use the new helper.

  This all still fits into commit 2's scope (it's the same "replace actor with handle" story, just landed
  without the extra trait). Shall I do it?

✻ Worked for 48s

❯ Couldn't we get rid of our own AcpConnectionHandle too?

● Yes — and it's worth doing. AcpConnectionHandle only exists because SessionManager is constructed before
   the ACP runtime hands you a real ConnectionTo<Client>, so we needed a slot to attach it into later.
  Strip that constraint and the handle dissolves into ConnectionTo<Client> itself, which is already Clone,
   cheap, and has the typed send_notification<N> / send_request<R> API.

  What the handle actually does, in decreasing order of necessity:

  ┌─────────────────────────────────────────────┬─────────────────────────────────────────────────────┐
  │               Responsibility                │                   Really needed?                    │
  ├─────────────────────────────────────────────┼─────────────────────────────────────────────────────┤
  │ Late-attach                                 │ Only because SessionManager::new runs before        │
  │ (OnceLock<ConnectionTo<Client>>)            │ connect_with's main_fn. Fixable.                    │
  ├─────────────────────────────────────────────┼─────────────────────────────────────────────────────┤
  │ Maps acp::Error → AcpServerError with an    │ One small free function does this.                  │
  │ operation label                             │                                                     │
  ├─────────────────────────────────────────────┼─────────────────────────────────────────────────────┤
  │ ConnectionUnavailable error variant         │ Only reachable before attach or after detach. Both  │
  │                                             │ disappear if we drop late-attach.                   │
  └─────────────────────────────────────────────┴─────────────────────────────────────────────────────┘

  The restructure (this extends commit 2's scope — still the same "remove the actor/handle indirection"
  story):

  - ACP handlers already receive cx: ConnectionTo<Client> as their third arg. Forward it into
  SessionManager methods: session_manager.new_session(req, &cx), prompt(req, &cx), etc. No field on
  SessionManager.
  - When SessionManager spawns a relay for a session, hand the relay cx.clone(). Relay stores its own
  ConnectionTo<Client> and uses it independently for the life of the session; when the client disconnects,
   sends fail naturally and the relay shuts down.
  - Replace AcpServerError::ConnectionUnavailable + AcpServerError::Protocol { operation, source } with
  just AcpServerError::Send { operation, source } (or keep both if there's still a reachable "no
  connection" path — I don't think there is).
  - A small fn map_send_err(op, e) -> AcpServerError free function does the error translation at the ~5
  call sites.
  - server/connection_handle.rs and server/connection.rs both get deleted. server/mod.rs exports only
  AcpServerError. The server feature becomes almost nothing — maybe just the error type and the one
  helper. Could even fold it up into the crate root.
  - testing.rs becomes a duplex-peer harness: fn test_connection() -> (ConnectionTo<Client>, TestPeer).

  What we lose: nothing structural. run_acp becomes slightly less symmetrical (no new_disconnected /
  attach ceremony — just construct SessionManager outside, and the handlers close over
  Arc<SessionManager>; each fired handler gets a real cx from the runtime).

  Scope check: this is bigger than what commit 2 touches. Files impacted beyond commit 2:
  - packages/aether-cli/src/acp/session_manager.rs — drop connection field, thread cx into methods.
  - packages/aether-cli/src/acp/relay.rs — hold ConnectionTo<Client> directly, error-map inline.
  - packages/aether-cli/src/acp/mappers.rs — replay_to_client takes &ConnectionTo<Client>.
  - packages/aether-cli/src/acp/handlers.rs — close over cx when dispatching to manager.
  - packages/aether-cli/src/acp/mod.rs — drop the new_disconnected/attach pair.
  - packages/aether-cli/src/acp/testing.rs — duplex peer instead of FakeAcpConnection.
  - Plus the corresponding test bodies in relay.rs / mappers.rs.

  A lot of those files were already heavily changed in the current staged working-tree, so we're rewriting
   half of them anyway. The incremental cost on top of what's already there is probably 30–60 min of
  focused work.

  Want me to unstage commit 2 (it's no longer the right target), make this change in-place, then
  re-propose a cleaner commit split? The new commit 2 would be something like "acp server: remove
  connection indirection, pass ConnectionTo through handlers", and it'd collapse server/ down to just the
  error type.

✻ Baked for 1m 19s

❯ Yes let's use ConnectionTo directly and rip out the uneeded abstractions, I've unstaged commit 2

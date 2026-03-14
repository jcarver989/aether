# ACP improvements for Aether + Wisp

## Summary

Aether and Wisp already cover the core ACP session lifecycle well: initialize, authenticate, new/load/list session, prompt, cancel, config options, session updates, embedded-context attachments, plans, and tool-call progress.

The most valuable missing work is mostly about **interoperability and generic ACP client/agent completeness**, not core functionality between Aether and Wisp.

## Prioritized recommendations

### 1. Advertise Aether's actual MCP transport support in `initialize`

**Priority:** High

This is the clearest correctness + interoperability win.

- `aether-cli` does not currently advertise `agentCapabilities.mcpCapabilities` in `initialize` (`packages/aether-cli/src/acp/session_manager.rs:383-397`)
- But it already accepts ACP MCP servers over **stdio, HTTP, and SSE** (`packages/aether-cli/src/acp/mappers.rs:67-94`)

Today an ACP client could conclude that Aether does not support HTTP/SSE MCP transports even though it actually does.

**Recommendation:** advertise:

- `mcpCapabilities.http = true`
- `mcpCapabilities.sse = true`

---

### 2. Make Wisp render standard ACP tool-call payloads, not just Aether-specific metadata

**Priority:** High

This is the biggest gap if Wisp is meant to be a strong **generic ACP client**.

Current Wisp tool-call handling mostly tracks:

- title
- raw input
- status
- optional custom result metadata

See `packages/wisp/src/components/tool_call_statuses.rs:188-247`.

What Wisp does **not** visibly handle from standard ACP tool calls:

- `content`
- `diff`
- `terminal`
- `locations`
- `rawOutput`

ACP tool calls are designed to carry rich structured output for diffs, live terminals, and follow-along file locations. Wisp is currently leaving a lot of that protocol value unused.

**Recommendation:** add first-class rendering for:

- standard content blocks in tool updates
- diff content
- terminal content references
- file locations / follow-along behavior
- raw output when present

---

### 3. Replace Wisp's unconditional auto-approval with real `session/request_permission` UX

**Priority:** High

Wisp currently uses `AutoApproveClient`, which automatically selects an allow option (`packages/acp-utils/src/client/session.rs:31-65`). Runtime setup wires that in directly (`packages/wisp/src/runtime_state.rs:27-35`).

ACP supports real permission prompting with:

- allow once / always
- reject once / always
- cancelled on prompt cancellation

So today Wisp effectively auto-approves requests by default.

**Recommendation:**

- surface permission requests in the UI
- let the user choose an option
- optionally keep auto-approve as a config toggle

**Nuance:** this matters more for Wisp as a generic ACP client than for the current Aether↔Wisp pairing, because Aether does not appear to rely heavily on ACP permission requests today.

---

### 4. Add dedicated session-mode compatibility (`modes`, `session/set_mode`, `current_mode_update`)

**Priority:** Medium

Aether clearly has a mode concept internally, but its ACP surface does not fully expose the dedicated session-mode API:

- `new_session` returns `config_options`, but not `modes` (`packages/aether-cli/src/acp/session_manager.rs:521-523`)
- `load_session` returns `config_options`, but not `modes` (`packages/aether-cli/src/acp/session_manager.rs:631`)
- `set_session_mode` returns `method_not_found` (`packages/aether-cli/src/acp/session_manager.rs:776-781`)

ACP docs note that dedicated session modes are being replaced by session config options, so this is **not** the highest-priority gap.

**Recommendation:** if broader ACP client compatibility matters, implement dedicated session modes as a thin compatibility layer over the existing mode config option system.

---

### 5. Add ACP terminal client methods in Wisp (`terminal/create`, `output`, `wait_for_exit`, `kill`, `release`)

**Priority:** Medium

Wisp does not currently advertise terminal capability in initialization (`packages/wisp/src/runtime_state.rs:27-35`), and there is no visible implementation of ACP terminal client methods.

ACP terminals are valuable because they let agents:

- run commands in the client environment
- stream output live
- embed terminals into tool calls
- manage timeouts / kill / release cleanly

**Recommendation:** implement terminal methods and pair them with UI support for terminal-backed tool calls.

---

### 6. Add ACP filesystem client methods in Wisp (`fs/read_text_file`, `fs/write_text_file`)

**Priority:** Medium-Low

Wisp also does not currently advertise ACP filesystem client capabilities (`packages/wisp/src/runtime_state.rs:27-35`).

These methods are most useful when:

- the agent is remote or sandboxed
- the client owns unsaved editor state
- edits should be mediated by the client rather than direct agent-side file access

That is less compelling for Wisp's current architecture as a TUI spawning a local subprocess agent.

**Recommendation:** add if Wisp is intended to become a more editor-like or generic ACP client.

---

### 7. Improve prompt content support: better `ResourceLink` handling and image support

**Priority:** Medium-Low

Current content mapping reduces richer ACP prompt content into basic placeholders:

- `ResourceLink` becomes a string like `[Resource: uri]`
- `Image` becomes `[Image content]`
- `Audio` becomes `[Audio content]`

See `packages/acp-utils/src/content.rs:7-18`.

Wisp attachment building currently embeds UTF-8 text resources, but skips binary / non-UTF8 files (`packages/wisp/src/components/app/attachments.rs:53-75`, especially `:61`).

So today:

- embedded text resources work well
- resource links are only minimally useful
- image prompts are effectively unsupported end-to-end
- audio is unsupported

**Recommendation:**

- add more meaningful `ResourceLink` resolution where safe and useful
- add image attachments / image prompt capability before audio
- do not prioritize audio unless there is a concrete product need

---

### 8. Add `session/list` pagination support (`cursor` / `nextCursor`)

**Priority:** Low

Aether supports cwd filtering in `session/list`, but appears to ignore pagination and never returns `nextCursor` (`packages/aether-cli/src/acp/session_manager.rs:529-550`).

This is mostly protocol completeness unless very large session histories are expected.

## Practical implementation order

If the goal is to invest where the payoff is highest, the best sequence is:

1. Advertise MCP HTTP/SSE support correctly
2. Upgrade Wisp tool-call rendering to standard ACP structures
3. Add real permission-request UX
4. Add session-mode compatibility if broader ACP client support matters
5. Add terminal methods
6. Add filesystem methods
7. Add richer content support (especially images)
8. Add pagination

## Bottom line

If the goal is strictly **Aether and Wisp working well together**, the only near-certain must-do is:

- **accurate MCP capability advertisement**

If the goal is **Aether as a stronger generic ACP agent** and **Wisp as a stronger generic ACP client**, then the top four improvements above are all worth doing.

## Protocol references

- https://agentclientprotocol.com/protocol/schema
- https://agentclientprotocol.com/protocol/session-modes
- https://agentclientprotocol.com/protocol/tool-calls
- https://agentclientprotocol.com/protocol/file-system
- https://agentclientprotocol.com/protocol/terminals
- https://agentclientprotocol.com/protocol/content

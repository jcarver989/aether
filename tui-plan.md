# TUI Crate Refactoring: Framework → Library

## Context

The `tui` crate currently has two personalities:

- **Library side** (clean): `Frame`, `Line`, `Span`, `Style`, `Layout`, `Panel`, `Component` trait, widgets — the caller creates them, calls methods, stays in control.
- **Framework side** (opinionated): `App` trait + `Runner` — the caller implements callbacks (`update`, `view`, `run_effect`), and the framework owns the event loop, terminal lifecycle, rendering pipeline, and effect processing.

The framework side creates friction for Wisp (the sole consumer):
- `run_effect` requires `&mut Renderer<impl Write>` for scrollback/clear/theme — coupling effects to the rendering layer
- Cursor positioning is manual and fragile: overlays and git diff bypass `Layout` and reconstruct `Frame` by hand (app/mod.rs:246-258, 271-290)
- The `advanced` module is an escape hatch for things the framework doesn't expose
- `prepare_for_view()` must be called manually before `view()` and after each effect — a framework smell leaking into consumer code

The goal: make the library API first-class, with the framework as a thin optional convenience layer on top.

---

## Design Principles

1. **Caller owns the loop.** The library provides building blocks; the caller decides when to read events, update state, and render.
2. **Compose, don't inherit.** Structs you own and call methods on, not traits you implement for a framework to call.
3. **No escape hatches.** If the "simple" API doesn't cover a use case, the underlying API should be the _same_ API, not a separate "advanced" module.
4. **Progressive disclosure.** Simple things should be simple. Complex things should be possible without switching paradigms.

---

## Ideal API (from first principles)

### Simplest possible app

```rust
#[tokio::main]
async fn main() -> io::Result<()> {
    let mut term = Terminal::new(TerminalConfig::default())?;
    let mut events = EventStream::new();
    let mut count = 0;

    loop {
        term.draw(|ctx| {
            Frame::new(vec![Line::new(format!("Count: {count}"))], Cursor::hidden())
        })?;

        match events.next().await {
            Some(TermEvent::Key(key)) if key.code == KeyCode::Char('q') => break,
            Some(TermEvent::Key(key)) if key.code == KeyCode::Char('j') => count += 1,
            Some(TermEvent::Resize(size)) => term.on_resize(size),
            None => break,
            _ => {}
        }
    }
    Ok(()) // Terminal restored on Drop
}
```

### With external events and effects

```rust
let mut term = Terminal::new(TerminalConfig { theme, ..Default::default() })?;
let mut events = EventStream::new();

loop {
    term.draw(|ctx| view(&state, ctx))?;

    tokio::select! {
        Some(event) = events.next() => {
            let actions = update(&mut state, event);
            for action in actions {
                match action {
                    Action::PushScrollback(lines) => term.push_to_scrollback(&lines)?,
                    Action::ClearScreen => term.clear_screen()?,
                    Action::FetchData => { /* async work */ },
                }
                term.draw(|ctx| view(&state, ctx))?; // re-render between effects
            }
        }
        Some(ext) = external_rx.recv() => {
            handle_external(&mut state, ext);
        }
    }
}
```

### With components

```rust
fn view(state: &AppState, ctx: &ViewContext) -> Frame {
    let mut layout = Layout::new();
    layout.section(state.conversation.render(ctx));
    layout.component(&state.prompt_composer, ctx);  // render + cursor in one call
    layout.section(status_line.render(ctx));
    layout.into_frame()
}
```

### Framework mode (optional, for simple apps)

```rust
// Still available for callers who want the Elm-style model
Runner::new(my_app).theme(theme).run().await?;
```

---

## Proposed Architecture

### Layer 1: Primitives (unchanged)
`Frame`, `Line`, `Span`, `Style`, `Size`, `Cursor`, `ViewContext`, `Theme`

Already clean value types. No changes needed.

### Layer 2: `Terminal` struct (new, replaces `advanced` module)

Unifies `TerminalSession` (RAII raw-mode guard) + `Renderer` (frame diffing) into one caller-owned struct:

```rust
pub struct Terminal<W: Write = io::Stdout> {
    _session: Option<TerminalSession>,  // None in test mode
    renderer: Renderer<W>,
}

impl Terminal {
    /// Enter raw mode and create a terminal writing to stdout.
    pub fn new(config: TerminalConfig) -> io::Result<Self>;
}

impl<W: Write> Terminal<W> {
    /// Create a terminal for testing (no raw mode).
    pub fn test(writer: W, theme: Theme) -> Self;

    /// Render a frame with automatic diffing.
    pub fn draw(&mut self, f: impl FnOnce(&ViewContext) -> Frame) -> io::Result<()>;

    /// Push lines to scrollback (above the managed viewport).
    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()>;

    /// Clear the entire screen.
    pub fn clear_screen(&mut self) -> io::Result<()>;

    /// Handle a terminal resize.
    pub fn on_resize(&mut self, size: impl Into<Size>);

    /// Get the current rendering context.
    pub fn context(&self) -> ViewContext;

    /// Change the active theme.
    pub fn set_theme(&mut self, theme: Theme);

    /// Current terminal size.
    pub fn size(&self) -> Size;
}
// Drop impl restores terminal via TerminalSession
```

```rust
pub struct TerminalConfig {
    pub theme: Theme,
    pub mouse_capture: MouseCapture,
    pub bracketed_paste: bool,
}
```

### Layer 3: `EventStream` (new, wraps spawn + channel)

```rust
pub struct EventStream {
    rx: mpsc::UnboundedReceiver<CrosstermEvent>,
}

impl EventStream {
    pub fn new() -> Self;
    pub async fn next(&mut self) -> Option<TermEvent>;
}

/// Cleaned-up event enum (no External/Tick — those are caller concerns)
pub enum TermEvent {
    Key(KeyEvent),
    Paste(String),
    Mouse(MouseEvent),
    Resize(Size),
}
```

Key difference from `AppEvent<E>`: no `External(E)` or `Tick`. Those are caller concerns, handled in the caller's `select!`. The library only produces terminal events.

### Layer 4: `Component` trait (refined)

```rust
pub trait Component {
    type Message;
    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>>;
    fn render(&self, ctx: &ViewContext) -> Vec<Line>;

    /// Cursor position within this component's rendered output.
    /// Default: hidden.
    fn cursor(&self, _ctx: &ViewContext) -> Cursor {
        Cursor::hidden()
    }
}
```

And `Layout` gains a convenience method:

```rust
impl Layout {
    /// Render a component and add it as a section with cursor tracking.
    pub fn component<C: Component>(&mut self, component: &C, ctx: &ViewContext) {
        let lines = component.render(ctx);
        let cursor = component.cursor(ctx);
        if cursor.is_visible {
            self.section_with_cursor(lines, cursor);
        } else {
            self.section(lines);
        }
    }
}
```

### Layer 5: Framework mode (optional, feature-gated under `runtime`)

`App` trait + `Runner` stay but are rewritten internally to use `Terminal`:

```rust
pub trait App {
    type Event;
    type Effect;
    type Error: From<io::Error>;

    fn update(&mut self, event: AppEvent<Self::Event>, ctx: &ViewContext) -> Option<Vec<Self::Effect>>;
    fn view(&self, ctx: &ViewContext) -> Frame;

    // Changed: &mut Terminal instead of &mut Renderer
    async fn run_effect(
        &mut self,
        terminal: &mut Terminal<impl Write>,
        effect: Self::Effect,
    ) -> Result<Vec<Self::Effect>, Self::Error> { ... }

    fn should_exit(&self) -> bool { false }
    fn wants_tick(&self) -> bool { false }
}
```

`Runner::run()` becomes a thin wrapper around the library-mode loop.

---

## Implementation Plan

### Phase 1: `Terminal` struct (additive, non-breaking)

**Files to create:**
- `packages/tui/src/terminal.rs` — new `Terminal<W>` struct + `TerminalConfig`

**Files to modify:**
- `packages/tui/src/lib.rs` — add `pub mod terminal;`, re-export `Terminal`, `TerminalConfig`

The `Terminal` struct composes the existing `TerminalSession` and `Renderer` internally. All methods delegate to them. This is purely additive — nothing existing changes.

Add `Cursor::hidden()` constructor if it doesn't exist (convenience for `Cursor { row: 0, col: 0, is_visible: false }`).

### Phase 2: `EventStream` (additive, non-breaking)

**Files to create:**
- `packages/tui/src/events.rs` — `EventStream` struct + `TermEvent` enum

**Files to modify:**
- `packages/tui/src/lib.rs` — add `pub mod events;`, re-export `EventStream`, `TermEvent`

`EventStream::new()` internally calls `spawn_terminal_event_task()` and wraps the receiver. `next()` converts `CrosstermEvent` → `TermEvent`, filtering out non-press key events and other noise (same logic currently in `run_loop`).

### Phase 3: `Component::cursor()` + `Layout::component()` (additive, non-breaking)

**Files to modify:**
- `packages/tui/src/components/component.rs` — add `cursor()` default method to `Component` trait
- `packages/tui/src/components/layout.rs` — add `Layout::component()` method
- `packages/tui/src/components/text_field.rs` — implement `cursor()` on `TextField`
- `packages/tui/src/components/form.rs` — implement `cursor()` on `Form`

Default `cursor()` returns hidden cursor, so all existing `Component` impls continue to compile unchanged.

### Phase 4: Migrate Wisp to `Terminal` (refactoring, consumer-side)

**Files to modify:**
- `packages/wisp/src/components/app/mod.rs`:
  - Change `run_effect` signature from `Renderer<impl Write>` to `Terminal<impl Write>`
  - Simplify `view()` for the normal case using `layout.component()`
- `packages/wisp/src/components/app/runtime.rs`:
  - Change `apply_action` to take `&mut Terminal<impl Write>` instead of `&mut Renderer<impl Write>`
  - Replace `terminal.push_to_scrollback()` → same method name, just different type
- `packages/wisp/src/components/prompt_composer.rs` — implement `cursor()` on the `Component` impl (it already has a `cursor()` method, just needs to move into the trait)
- `packages/wisp/src/main.rs` — no changes yet (still uses `Runner`)

### Phase 5: Update `App` trait + deprecate `advanced` (breaking)

**Files to modify:**
- `packages/tui/src/runtime/app.rs`:
  - Change `run_effect` default signature to `&mut Terminal<impl Write>`
  - Rewrite `Runner::run()` to use `Terminal` + `EventStream` internally
  - Rewrite `run_loop` to use `Terminal` internally
- `packages/tui/src/lib.rs`:
  - Deprecate `advanced` module (still accessible but marked `#[deprecated]`)
  - `Terminal` and `EventStream` become the primary public API alongside primitives

### Phase 6 (future, optional): Migrate Wisp off `Runner`

If desired, Wisp can drop the `App` trait entirely and own its event loop directly using `Terminal` + `EventStream` + `tokio::select!`. This eliminates:
- The `App` trait implementation
- The effect/follow-up-effect queuing
- The `prepare_for_view()` pattern (just call it when you need it)
- The `should_exit()` / `wants_tick()` polling

This is optional — the `App` + `Runner` pattern is fine for many apps. But having the library-mode API available means Wisp can choose.

---

## What This Solves

| Pain Point | Solution |
|---|---|
| `run_effect` takes `&mut Renderer` (framework coupling) | Takes `&mut Terminal` — clean, first-class API |
| `advanced` module is an escape hatch | `Terminal` IS the primary API; `advanced` deprecated |
| Manual cursor in overlays / git diff (mod.rs:246-290) | `Component::cursor()` + `Layout::component()` |
| `prepare_for_view()` called manually | Still needed with App trait, but eliminated if caller owns the loop |
| Combobox not a Component | Not addressed here (separate concern, can add `handle_key()` independently) |
| Multi-level message routing | Inherent to Elm architecture — not a library concern |

## What Stays the Same

- `Line`/`Span`/`Style`/`Frame`/`Size`/`Cursor` — unchanged
- `Component` trait shape — only adds optional `cursor()` default
- `Renderer` internals — frame diffing, `VisualFrame`, `TerminalScreen` unchanged
- `FocusRing`, `Panel`, `Layout`, all widgets — unchanged
- Feature gates (`syntax`, `runtime`, `picker`, `testing`) — unchanged
- `App` + `Runner` — still available, just rewritten internally

---

## Verification

1. **Unit tests**: `Terminal::test(Vec::new(), theme)` enables testing without a real terminal — same pattern as current `Renderer::new(Vec::new(), theme)`
2. **Existing tests pass**: Phases 1-3 are additive; Phase 4 changes Wisp types but not behavior; Phase 5 changes `App` trait signature (compile-time verification)
3. **Counter example**: Update `examples/counter.rs` to use library-mode API as documentation
4. **`cargo test` across workspace**: `cargo test -p tui && cargo test -p wisp`
5. **Manual smoke test**: Run wisp, verify rendering, scrollback, theme switching, git diff viewer, config overlay all work

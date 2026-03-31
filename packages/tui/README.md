# tui

A lightweight, composable terminal UI library for building full-screen CLI apps in Rust.

Your app owns its event loop and state machine. The library provides composable building blocks: a [`Component`] trait for widgets, a diff-based [`Renderer`], and RAII terminal management.

## Minimal app

A complete TUI app has four parts: a [`TerminalSession`] (raw mode guard), a [`Renderer`] (output), an event source, and a loop that wires them together.

```rust,no_run
use std::io;
use tui::{
    Component, CrosstermEvent, Event, Frame, KeyCode, Line,
    MouseCapture, Renderer, TerminalSession, Theme, ViewContext,
    spawn_terminal_event_task, terminal_size,
};

// 1. Define your root component
struct Counter { count: i32 }

impl Component for Counter {
    type Message = CounterMsg;
    async fn on_event(&mut self, event: &Event) -> Option<Vec<CounterMsg>> {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up    => self.count += 1,
                KeyCode::Down  => self.count -= 1,
                KeyCode::Char('q') => return Some(vec![CounterMsg::Quit]),
                _ => return None,
            }
            return Some(vec![]);
        }
        None
    }
    fn render(&mut self, ctx: &ViewContext) -> Frame {
        Frame::new(vec![
            Line::styled("Counter (↑/↓, q to quit)", ctx.theme.muted()),
            Line::new(format!("  {}", self.count)),
        ])
    }
}

enum CounterMsg { Quit }

// 2. Set up terminal, renderer, and event source
#[tokio::main]
async fn main() -> io::Result<()> {
    let size = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(io::stdout(), Theme::default(), size);
    let _session = TerminalSession::new(true, MouseCapture::Disabled)?;
    let mut events = spawn_terminal_event_task();

    let mut app = Counter { count: 0 };
    renderer.render_frame(|ctx| app.render(ctx))?; // initial paint

    // 3. Event loop
    loop {
        let Some(raw) = events.recv().await else { break };
        if let CrosstermEvent::Resize(c, r) = &raw {
            renderer.on_resize((*c, *r));
        }
        if let Ok(event) = Event::try_from(raw) {
            if let Some(msgs) = app.on_event(&event).await {
                for msg in msgs {
                    match msg {
                        CounterMsg::Quit => return Ok(()),
                    }
                }
            }
            renderer.render_frame(|ctx| app.render(ctx))?;
        }
    }
    Ok(())
}
```

Dropping `_session` automatically restores the terminal (disables raw mode, bracketed paste, and mouse capture).

## How it works

```text
crossterm::Event ──→ Event::try_from ──→ Component::on_event ──→ Vec<Message>
                                                │                       │
                                                ▼                       ▼
                                         Component::render     parent handles messages
                                                │
                                                ▼
                                    Renderer::render_frame (diff → ANSI)
```

1. **[`spawn_terminal_event_task()`]** reads raw crossterm events in a blocking tokio task.
2. **[`Event::try_from`]** filters key releases and normalizes resize events.
3. **[`Component::on_event`]** returns `None` (ignored), `Some(vec![])` (consumed), or `Some(vec![msg])` (messages for the parent).
4. **[`Component::render`]** returns a [`Frame`] (lines + cursor) given a [`ViewContext`] (size + theme).
5. **[`Renderer::render_frame`]** diffs against the previous frame and emits only changed ANSI sequences.

## Composing components

Nest components by owning them in your parent and delegating events:

```rust,no_run
use tui::{Component, Event, Frame, Layout, ViewContext, TextField, merge};

struct MyApp {
    name: TextField,
    path: TextField,
    // ...
}

impl Component for MyApp {
    type Message = ();
    async fn on_event(&mut self, event: &Event) -> Option<Vec<()>> {
        // Delegate to the focused child; merge results if needed
        merge(
            self.name.on_event(event).await,
            self.path.on_event(event).await,
        )
    }
    fn render(&mut self, ctx: &ViewContext) -> Frame {
        // Stack child frames vertically
        let mut layout = Layout::new();
        layout.section(self.name.render(ctx).into_lines());
        layout.section(self.path.render(ctx).into_lines());
        layout.into_frame()
    }
}
```

Use [`FocusRing`] to track which child receives events and [`Layout`] to stack frames vertically.

## Built-in widgets

| Widget | Description |
|--------|-------------|
| [`TextField`] | Single-line text input |
| [`NumberField`] | Numeric input (integer or float) |
| [`Checkbox`] | Boolean toggle `[x]` / `[ ]` |
| [`RadioSelect`] | Single-select radio buttons |
| [`MultiSelect`] | Multi-select checkboxes |
| [`SelectList`] | Scrollable list with selection |
| [`Form`] | Multi-field tabbed form |
| [`Panel`] | Bordered container |
| [`Spinner`] | Animated progress indicator |
| [`Combobox`] | Fuzzy-searchable picker (feature: `picker`) |

## Feature flags

| Feature | Description | Default |
|---------|-------------|---------|
| `syntax` | Syntax highlighting, markdown rendering, diff previews via syntect | yes |
| `picker` | Fuzzy combobox picker via nucleo | yes |
| `testing` | Test utilities ([`TestTerminal`](testing::TestTerminal), `render_component`, `assert_buffer_eq`) | no |

Disable defaults for a smaller dependency tree:

```toml
[dependencies]
tui = { version = "0.1", default-features = false }
```

## License

MIT

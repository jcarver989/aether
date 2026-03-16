# tui

A lightweight, composable terminal UI library for building full-screen CLI apps.

## Core Primitives

- **`Component`** — trait for reusable widgets with event handling and rendering
- **`Event`** — unified input events (key, paste, mouse, tick, resize)
- **`Frame`** — rendered output: lines + cursor position
- **`Layout`** — vertical section composition into a `Frame`
- **`Renderer`** — efficient diff-based terminal renderer (feature: `runtime`)
- **`TerminalSession`** — raw-mode lifecycle management (feature: `runtime`)

## Quick Start

The library provides composable building blocks — your app owns its event loop and state machine.

```rust
use tui::{Component, Cursor, Event, Frame, KeyCode, Layout, Line, Renderer, TerminalSession, ViewContext};
use tui::spawn_terminal_event_task;

// Define your app state and use Component for child widgets
struct MyWidget { count: i32 }

impl Component for MyWidget {
    type Message = ();
async fn on_event(&mut self, event: &Event) -> Option<Vec<()>> {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('j') => self.count += 1,
                KeyCode::Char('k') => self.count -= 1,
                _ => return None,
            }
        }
        Some(vec![])
    }
    fn render(&self, _ctx: &ViewContext) -> Vec<Line> {
        vec![Line::new(format!("Count: {}", self.count))]
    }
}
```

Convert crossterm events with `Event::try_from(crossterm_event)` — it filters key releases and maps resize events automatically.

## Built-in Widgets

- `Panel` — bordered container
- `Form`, `TextField`, `NumberField` — form inputs
- `Checkbox`, `RadioSelect`, `MultiSelect` — selection controls
- `SelectList` — scrollable list with selection
- `Spinner` — animated progress indicator
- `Combobox` — fuzzy-searchable picker (feature: `picker`)
- `FocusRing` — Tab/BackTab focus traversal

## Feature Flags

| Feature | Description | Default |
|---|---|---|
| `syntax` | Syntax highlighting via syntect | yes |
| `runtime` | Terminal renderer, session management, event task | yes |
| `picker` | Fuzzy combobox picker | yes |
| `testing` | Test utilities (`TestTerminal`, `render_component`) | no |

Disable defaults for lower-level use:

```toml
[dependencies]
tui = { version = "0.1", default-features = false }
```

## License

MIT

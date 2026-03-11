# tui

A lightweight, composable terminal UI framework for building full-screen CLI apps.

## Start here

The primary app model has four pieces:

- `App` — your application state machine
- `AppEvent` — unified terminal, external, and tick events
- `Effects` — follow-up work or exit
- `Runner` — terminal bootstrap and event loop ownership

A minimal app looks like this:

```rust
use tui::{App, AppEvent, Cursor, Effects, Frame, KeyCode, Line, ViewContext, Runner};

struct Counter {
    count: i32,
}

impl App for Counter {
    type Event = ();
    type Effect = ();
    type Error = std::io::Error;

    fn update(&mut self, event: AppEvent<Self::Event>, _ctx: &ViewContext) -> Effects<Self::Effect> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => Effects::exit(),
            AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                self.count += 1;
                Effects::none()
            }
            AppEvent::Key(key) if key.code == KeyCode::Char('k') => {
                self.count -= 1;
                Effects::none()
            }
            _ => Effects::none(),
        }
    }

    fn view(&self, _ctx: &ViewContext) -> Frame {
        Frame::new(
            vec![Line::new(format!("Count: {}", self.count))],
            Cursor {
                row: 0,
                col: 0,
                is_visible: false,
            },
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    Runner::new(Counter { count: 0 }).run().await
}
```

See:

- `examples/counter.rs` for the minimal happy path
- `examples/child_widgets.rs` for reusable child widgets
- `examples/manual_runtime.rs` for advanced manual runtime control

### Runner configuration

Use builder methods when you need to customize the default runtime:

```rust,no_run
# use std::time::Duration;
# use tui::{App, AppEvent, Cursor, Effects, Frame, Line, ViewContext, Runner};
# struct MyApp;
# impl App for MyApp {
#     type Event = (); type Effect = (); type Error = std::io::Error;
#     fn update(&mut self, _event: AppEvent<Self::Event>, _ctx: &ViewContext) -> Effects<Self::Effect> { Effects::none() }
#     fn view(&self, _ctx: &ViewContext) -> Frame {
#         Frame::new(vec![Line::new("")], Cursor { row: 0, col: 0, is_visible: false })
#     }
# }
# async fn example(app: MyApp, rx: tokio::sync::mpsc::UnboundedReceiver<()>) -> Result<(), std::io::Error> {
Runner::new(app)
    .tick_rate(Duration::from_millis(100))
    .external_events(rx)
    .run()
    .await
# }
```

## Reusable widgets

Use `Component` for rendering reusable child widgets and `InteractiveComponent` when a child widget needs to emit messages back to its parent.

```rust
use tui::{Component, InteractiveComponent, Line, MessageResult, ViewContext, UiEvent};

struct Greeting {
    name: String,
}

impl Component for Greeting {
    fn render(&self, _ctx: &ViewContext) -> Vec<Line> {
        vec![Line::new(format!("Hello, {}!", self.name))]
    }
}

impl InteractiveComponent for Greeting {
    type Message = ();

    fn on_event(&mut self, _event: UiEvent) -> MessageResult<Self::Message> {
        MessageResult::ignored()
    }
}
```

Focus helpers:

- `FocusRing` for simple Tab / BackTab traversal
- `FocusGroup` for higher-level focus routing and scope boundaries

Useful building blocks:

- `Panel`
- `Form`
- `TextField`
- `Checkbox`
- `RadioSelect`
- `MultiSelect`
- `Dialog`
- `Spinner`
- `StatusBar`

## Advanced control

If you need to manage terminal setup or the event loop yourself, use `tui::advanced`.

```rust
use tui::advanced::{run_app, Action, Renderer, RootApp, RootComponent, TerminalSession};
```

Advanced APIs are for cases where you want manual renderer control, explicit terminal lifecycle management, or the legacy runtime path. Most applications should prefer `App` + `Runner`.

## Feature flags

| Feature | Description | Default |
|---|---|---|
| `syntax` | Syntax highlighting via syntect | ✅ |
| `markdown` | Markdown rendering | ✅ |
| `diff` | Diff preview rendering | ✅ |
| `serde` | Form JSON serialization helpers | ✅ |
| `runtime` | Async terminal runtime | ✅ |
| `picker` | Fuzzy combobox picker | ✅ |
| `testing` | Test utilities | ❌ |

Disable defaults when you only need lower-level primitives:

```toml
[dependencies]
tui = { version = "0.1", default-features = false }
```

## License

MIT

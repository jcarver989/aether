# tui

A lightweight, composable terminal UI framework for building rich CLI applications.

## Features

- **Component model** — `Component`, `InteractiveComponent`, `RootComponent`, and typed actions
- **Styled text** — `Line`/`Span`/`Style` primitives with ANSI output, unicode-width aware
- **Frame-diffing renderer** — `TerminalScreen` diffs frames and only rewrites changed lines; `Renderer` adds soft-wrap, cursor management, and progressive scrollback overflow
- **Focus management** — `FocusRing` for Tab/BackTab cycling across child components
- **Theme system** — Semantic color palettes derived from `.tmTheme` files
- **Built-in widgets** — `TextField`, `NumberField`, `Checkbox`, `RadioSelect`, `MultiSelect`, `Form`, `Spinner`, `Combobox`
- **Runtime entrypoint** — `tui::run_app(...)` + `RuntimeApp` for trait-based application runtime control

## Rendering primitives

```rust
use tui::{Component, Line, RenderContext};

struct Greeting {
    name: String,
}

impl Component for Greeting {
    fn render(&self, _ctx: &RenderContext) -> Vec<Line> {
        vec![Line::new(format!("Hello, {}!", self.name))]
    }
}
```

## Running an application

`tui` uses a trait-based runtime API:
- `run_app(...)` as the main entrypoint
- `RuntimeApp` for app-owned event/effect handling

```rust,no_run
use std::error::Error;
use tui::{
    Cursor, Frame, RenderContext, RootComponent, RuntimeAction, RuntimeApp, RuntimeEvent,
    RuntimeOptions, run_app,
};

struct App {
    value: usize,
}

enum Effect {
    Save,
}

impl RootComponent for App {
    fn render(&mut self, _context: &RenderContext) -> Frame {
        Frame::new(
            vec![tui::Line::new(format!("value: {}", self.value))],
            Cursor {
                row: 0,
                col: 0,
                is_visible: false,
            },
        )
    }
}

impl RuntimeApp for App {
    type External = ();
    type Effect = Effect;
    type Error = Box<dyn Error>;

    fn on_event(
        &mut self,
        event: RuntimeEvent<Self::External>,
        _context: &RenderContext,
    ) -> Vec<RuntimeAction<Self::Effect>> {
        match event {
            RuntimeEvent::Terminal(crossterm::event::Event::Key(_)) => {
                self.value += 1;
                vec![RuntimeAction::Render, RuntimeAction::Effect(Effect::Save)]
            }
            RuntimeEvent::Tick(_) => vec![RuntimeAction::Render],
            RuntimeEvent::External(_) => vec![],
            RuntimeEvent::Terminal(_) => vec![],
        }
    }

    async fn on_effect<W: std::io::Write>(
        &mut self,
        _renderer: &mut tui::Renderer<W>,
        effect: Self::Effect,
    ) -> Result<Vec<RuntimeAction<Self::Effect>>, Self::Error> {
        match effect {
            Effect::Save => Ok(vec![]),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App { value: 0 };

    run_app(
        &mut app,
        None::<tokio::sync::mpsc::UnboundedReceiver<()>>,
        RuntimeOptions::default(),
    )
    .await
}
```

The high-level runtime centers on `run_app(...)`, with `RuntimeApp` keeping event and
effect handling on the app itself. Low-level APIs like `Renderer` and
`spawn_terminal_event_task()` remain available for advanced consumers that want full
control over terminal lifecycle or event plumbing.

External async event sources are supported via an optional
`tokio::sync::mpsc::UnboundedReceiver<_>` passed to `run_app(...)`.

## Feature flags

| Feature    | Default | Description                                      |
|------------|---------|--------------------------------------------------|
| `markdown` | yes     | Markdown rendering (pulls in `pulldown-cmark`)   |
| `serde`    | yes     | JSON serialization for form values (`serde_json`) |
| `runtime`  | yes     | Runtime entrypoint and terminal event handling   |
| `picker`   | yes     | Fuzzy-search combobox (`nucleo`)                 |

Disable defaults with `default-features = false` and enable only what you need:

```toml
[dependencies]
tui = { version = "0.1", default-features = false, features = ["markdown"] }
```

## License

MIT

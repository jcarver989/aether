# tui

A lightweight, composable terminal UI framework for building rich CLI applications.

## Features

- **Component model** — `Component` trait for rendering, `HandlesInput` for typed keyboard actions, `Tickable` for animations
- **Styled text** — `Line`/`Span`/`Style` primitives with ANSI output, unicode-width aware
- **Frame-diffing renderer** — `Screen` diffs frames and only rewrites changed lines; `Renderer` adds soft-wrap, cursor management, and progressive scrollback overflow
- **Focus management** — `FocusRing` for Tab/BackTab cycling across child components
- **Theme system** — Semantic color palettes derived from `.tmTheme` files (ships with Catppuccin Mocha)
- **Built-in widgets** — `TextField`, `NumberField`, `Checkbox`, `RadioSelect`, `MultiSelect`, `Form`, `Spinner`, `Combobox`

## Quick start

```rust
use tui::{Component, RenderContext, Line};

struct Greeting { name: String }

impl Component for Greeting {
    fn render(&self, _ctx: &RenderContext) -> Vec<Line> {
        vec![Line::new(format!("Hello, {}!", self.name))]
    }
}
```

## Feature flags

| Feature    | Default | Description                                         |
|------------|---------|-----------------------------------------------------|
| `markdown` | yes     | Markdown rendering (pulls in `pulldown-cmark`)      |
| `serde`    | yes     | JSON serialization for form values (`serde_json`)   |
| `runtime`  | yes     | Terminal event task spawning (`tokio`)               |
| `picker`   | yes     | Fuzzy-search combobox (`nucleo`)                    |

Disable defaults with `default-features = false` and enable only what you need:

```toml
[dependencies]
tui = { version = "0.1", default-features = false, features = ["markdown"] }
```

## License

MIT

The embedded `catppuccin-mocha.tmTheme` is from the [Catppuccin](https://github.com/catppuccin/catppuccin) project, also MIT licensed.

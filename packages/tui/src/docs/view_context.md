Environment passed to [`Component::render`](crate::Component::render): terminal size and theme.

Every render call receives a `ViewContext` so components can adapt to the available space and use semantic colors from the [`Theme`].

# Construction

```rust,no_run
use tui::ViewContext;

// Default theme
let ctx = ViewContext::new((80, 24));

// Custom theme
use tui::Theme;
let ctx = ViewContext::new_with_theme((120, 40), Theme::default());
```

# Methods

- **`with_size(size)`** — Clone the context with a different `Size`, preserving the theme (and syntax highlighter). Useful when rendering a child into a sub-region.
- **`highlighter()`** — Access the [`SyntaxHighlighter`](crate::SyntaxHighlighter) (requires feature `syntax`).

# `Size`

`Size` holds terminal dimensions as `width` (columns) and `height` (rows), both `u16`. Implements `From<(u16, u16)>` for convenient construction.

# See also

- [`Theme`] — The semantic color palette.
- [`Renderer`](crate::Renderer) — Creates `ViewContext` automatically from its own state.

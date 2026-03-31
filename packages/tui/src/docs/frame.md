Logical output from a [`Component::render`](crate::Component::render) call: a vector of [`Line`]s plus a [`Cursor`] position.

A `Frame` is a pure data structure with no terminal formatting. The [`Renderer`](crate::Renderer) consumes frames, diffs them against the previous render, and emits only the changed ANSI sequences.

# Construction

```rust,no_run
use tui::{Frame, Line};
use tui::Cursor;

let frame = Frame::new(vec![
    Line::new("Hello, world!"),
    Line::new("Press q to quit"),
]);

// Optionally place a visible cursor
let frame = frame.with_cursor(Cursor::visible(0, 5));
```

# Methods

- **`lines()`** — Borrow the rendered lines.
- **`cursor()`** — The current cursor state.
- **`with_cursor(cursor)`** — Replace the cursor (builder pattern, moves `self`).
- **`clamp_cursor()`** — Clamp the cursor row to the last line index, preventing out-of-bounds positions.
- **`into_lines()`** — Consume the frame and return the lines.
- **`into_parts()`** — Consume and return `(Vec<Line>, Cursor)`.

# See also

- [`Cursor`] — Logical cursor with row, col, and visibility.
- [`Line`] — A single line of styled terminal output.
- [`Renderer`](crate::Renderer) — Consumes frames and renders them to the terminal.
- [`Layout`](crate::Layout) — Composes multiple frames vertically.

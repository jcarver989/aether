  What tui/ would need to become a standalone open-sourceable crate

  What's already strong

  - Clean component model — Component, HandlesInput<Action>, Tickable, InputOutcome<A> form a cohesive,
  composable system
  - Good test coverage — Every component has unit tests, screen diffing has solid coverage, markdown
  rendering is well-tested
  - Zero async in the core — The component/render layer is synchronous (only runtime.rs uses tokio), which
  is good for a library
  - Smart rendering — Screen does frame diffing, Renderer handles soft-wrap + progressive scrollback
  overflow, synchronized updates
  - Solid primitives — Line/Span/Style is a clean styled-text model, unicode-width aware throughout

  Completed

  1. ✅ Sever external coupling (theme→settings, diff→acp_utils, test helpers)
  2. ✅ Feature-gate heavy deps (syntect, pulldown-cmark, nucleo, tokio, serde_json)
  3. ✅ Fix Component::render(&mut self) → &self
  4. ✅ Add a Size struct and clean up RenderContext
  5. ✅ Add reusable FocusRing with Tab/BackTab cycling
  6. ✅ Add map/discard_action to InputOutcome
  7. ✅ Refactor Form to use FocusRing
  8. ✅ Add crate-level docs, doc comments on public types, README

  Remaining (nice-to-have for future versions)

  - No scroll container / viewport — There's no generic scrollable region component. The Renderer handles
  overflow but only at the top level.
  - No layout system — No horizontal/vertical split, flex, or constraint-based layout. Components just
  return Vec<Line> and callers manually compose.
  - No mouse support — Input is keyboard-only (KeyEvent). No MouseEvent handling.
  - No text wrapping in components — TextField has no cursor movement (can only append/backspace from end),
   no selection, no multi-line editing.
  - No accessibility — No screen reader support, no semantic annotations.
  - Span fields are private but Style fields are public — Inconsistent visibility.
  - No error type — Components can't signal errors during rendering or input handling.

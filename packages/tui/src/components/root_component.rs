pub use crate::rendering::frame::{Cursor, Frame};
pub use crate::rendering::render_context::RenderContext;

/// A component that renders a full frame with cursor information for the [`Renderer`](crate::Renderer).
///
/// The rendering pipeline separates *what determines if we render* ([`Props`](RootComponent::Props))
/// from *how we render efficiently* (`&self` cache access):
///
/// 1. [`props()`](RootComponent::props) — called after every event; refreshes caches and returns
///    a lightweight, comparable snapshot of render-relevant state.
/// 2. The runtime compares the returned props with the previous value (`PartialEq`).
/// 3. If changed, [`render()`](RootComponent::render) produces the frame using `&self` for
///    read-only cache access and the props for input data.
pub trait RootComponent {
    /// Lightweight, comparable snapshot of the state that determines rendering.
    ///
    /// Must be cheap to compare (`PartialEq`) and clone. Heavy data (rendered
    /// line caches, syntax highlighting) lives on `&self`, not here.
    type Props: Clone + PartialEq;

    /// Derive current render props. May refresh internal caches.
    ///
    /// Called by the runtime after every event to determine if rendering is needed.
    /// All mutation is isolated to this method; [`render()`](RootComponent::render) is `&self`.
    fn props(&mut self, context: &RenderContext) -> Self::Props;

    /// Produce a frame from props. `&self` provides read-only access to caches
    /// populated during [`props()`](RootComponent::props).
    fn render(&self, props: &Self::Props, context: &RenderContext) -> Frame;
}

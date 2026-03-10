pub mod checkbox;
pub mod component;
pub mod container;
pub mod dialog;
pub mod form;
pub mod interactive_component;
pub mod multi_select;
pub mod number_field;
pub mod radio_select;
pub mod select_option;
pub mod spinner;
pub mod status_bar;
pub mod text_field;

// Re-export the core traits and types
pub use component::{Component, RenderContext};
pub use container::{BORDER_H_PAD, Container};
pub use interactive_component::{InteractiveComponent, MessageResult, UiEvent};
pub use crate::rendering::frame::{Cursor, Frame};

/// Wrapping navigation helper for selection indices.
/// `delta` of -1 moves up, +1 moves down, wrapping at boundaries.
pub fn wrap_selection(index: &mut usize, len: usize, delta: isize) {
    if len == 0 {
        return;
    }
    *index = ((*index).cast_signed() + delta).rem_euclid(len.cast_signed()) as usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_selection_wraps_up_from_zero() {
        let mut idx = 0;
        wrap_selection(&mut idx, 3, -1);
        assert_eq!(idx, 2);
    }

    #[test]
    fn wrap_selection_wraps_down_from_last() {
        let mut idx = 2;
        wrap_selection(&mut idx, 3, 1);
        assert_eq!(idx, 0);
    }

    #[test]
    fn wrap_selection_noop_on_empty() {
        let mut idx = 0;
        wrap_selection(&mut idx, 0, 1);
        assert_eq!(idx, 0);
    }

    #[test]
    fn wrap_selection_moves_normally() {
        let mut idx = 1;
        wrap_selection(&mut idx, 5, 1);
        assert_eq!(idx, 2);
        wrap_selection(&mut idx, 5, -1);
        assert_eq!(idx, 1);
    }
}

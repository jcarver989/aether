pub mod app;
pub mod command_picker;
pub mod config_menu;
pub mod config_overlay;
pub mod config_picker;
pub mod container;
pub mod conversation_window;
pub mod elicitation_form;
pub mod file_picker;
pub mod input_prompt;
pub mod progress_indicator;
pub mod server_status;
pub mod status_line;
pub mod text_input;
pub mod thought_message;
pub mod tool_call_statuses;

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

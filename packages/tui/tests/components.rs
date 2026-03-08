#[path = "components/checkbox.rs"]
mod checkbox;
#[path = "components/multi_select.rs"]
mod multi_select;
#[path = "components/number_field.rs"]
mod number_field;
#[path = "components/radio_select.rs"]
mod radio_select;
#[path = "components/spinner.rs"]
mod spinner;
#[path = "components/text_field.rs"]
mod text_field;

use tui::testing::{
    assert_buffer_eq, key, render_component, render_component_with_screen, render_lines,
    sample_options, TestTerminal,
};
use tui::{Component, HandlesInput, RenderContext};

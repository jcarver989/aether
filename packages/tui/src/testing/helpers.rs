use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::rendering::frame::Cursor;
use crate::rendering::frame::Frame;
use crate::rendering::renderer::Renderer;
use crate::{SelectOption, ViewContext};

use super::TestTerminal;

fn frame_from_lines(lines: &[crate::line::Line], _width: u16, _rows: u16) -> Frame {
    Frame::new(lines.to_vec())
        .with_cursor(Cursor {
            row: lines.len().saturating_sub(1),
            col: 0,
            is_visible: true,
        })
        .clamp_cursor()
}

pub fn render_component(
    render: impl Fn(&ViewContext) -> Frame,
    width: u16,
    rows: u16,
) -> TestTerminal {
    let ctx = ViewContext::new((width, rows));
    let frame = render(&ctx);
    let terminal = TestTerminal::new(width, rows);
    let mut renderer = Renderer::new(terminal, crate::theme::Theme::default());
    renderer.on_resize((width, rows));
    renderer.render_frame(|_| frame).unwrap();
    renderer.writer().clone()
}

pub fn render_component_with_renderer(
    render: impl Fn(&ViewContext) -> Frame,
    renderer: &mut Renderer<TestTerminal>,
    width: u16,
    rows: u16,
) {
    let ctx = ViewContext::new((width, rows));
    let frame = render(&ctx);
    renderer.render_frame(|_| frame).unwrap();
}

pub fn render_lines(lines: &[crate::line::Line], width: u16, rows: u16) -> TestTerminal {
    render_component(|_| frame_from_lines(lines, width, rows), width, rows)
}

pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

pub fn sample_options() -> Vec<SelectOption> {
    vec![
        SelectOption {
            value: "a".into(),
            title: "Alpha".into(),
        },
        SelectOption {
            value: "b".into(),
            title: "Beta".into(),
        },
        SelectOption {
            value: "c".into(),
            title: "Gamma".into(),
        },
    ]
}

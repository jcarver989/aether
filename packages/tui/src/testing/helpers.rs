use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::rendering::frame::Cursor;
use crate::rendering::frame::Frame;
use crate::rendering::size::Size;
use crate::rendering::terminal_screen::TerminalScreen;
use crate::{Component, RenderContext, SelectOption};

use super::TestTerminal;

fn frame_from_lines(lines: &[crate::line::Line], width: u16, _rows: u16) -> Frame {
    Frame::new(
        lines.to_vec(),
        Cursor {
            row: lines.len().saturating_sub(1),
            col: 0,
            is_visible: true,
        },
    )
    .soft_wrap(width)
    .clamp_cursor()
}

pub fn render_component(component: &impl Component, width: u16, rows: u16) -> TestTerminal {
    let ctx = RenderContext::new((width, rows));
    let lines = component.render(&ctx);
    let terminal = TestTerminal::new(width, rows);
    let mut screen = TerminalScreen::new(terminal);
    let frame = frame_from_lines(&lines, width, rows).prepare(Size::from((width, rows)), 0);
    screen.render_frame(&frame, width).unwrap();
    screen.writer().clone()
}

pub fn render_component_with_terminal_state(
    component: &impl Component,
    terminal_state: &mut TerminalScreen<TestTerminal>,
    width: u16,
    rows: u16,
) {
    let ctx = RenderContext::new((width, rows));
    let lines = component.render(&ctx);
    let frame = frame_from_lines(&lines, width, rows).prepare(Size::from((width, rows)), 0);
    terminal_state.render_frame(&frame, width).unwrap();
}

pub fn render_lines(lines: &[crate::line::Line], width: u16, rows: u16) -> TestTerminal {
    let terminal = TestTerminal::new(width, rows);
    let mut terminal_state = TerminalScreen::new(terminal);
    let frame = frame_from_lines(lines, width, rows).prepare(Size::from((width, rows)), 0);
    terminal_state.render_frame(&frame, width).unwrap();
    terminal_state.writer().clone()
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

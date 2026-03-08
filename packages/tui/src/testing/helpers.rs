use std::io::Write;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::screen::Screen;
use crate::{Component, RenderContext, SelectOption};

use super::TestTerminal;

pub fn render_component(component: &impl Component, width: u16, rows: u16) -> TestTerminal {
    let ctx = RenderContext::new((width, rows));
    let lines = component.render(&ctx);
    let mut terminal = TestTerminal::new(width, rows);
    let mut screen = Screen::new();
    screen.render(&lines, width, &mut terminal).unwrap();
    terminal.flush().unwrap();
    terminal
}

pub fn render_component_with_screen(
    component: &impl Component,
    screen: &mut Screen,
    terminal: &mut TestTerminal,
    width: u16,
    rows: u16,
) {
    let ctx = RenderContext::new((width, rows));
    let lines = component.render(&ctx);
    screen.render(&lines, width, terminal).unwrap();
    terminal.flush().unwrap();
}

pub fn render_lines(
    lines: &[crate::rendering::screen::Line],
    width: u16,
    rows: u16,
) -> TestTerminal {
    let mut terminal = TestTerminal::new(width, rows);
    let mut screen = Screen::new();
    screen.render(lines, width, &mut terminal).unwrap();
    terminal.flush().unwrap();
    terminal
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

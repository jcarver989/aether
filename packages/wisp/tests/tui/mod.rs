mod checkbox;
mod multi_select;
mod number_field;
mod radio_select;
mod spinner;
mod text_field;

use std::io::Write;

use super::test_terminal::{TestTerminal, assert_buffer_eq};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use wisp::tui::screen::Screen;
use wisp::tui::{Component, HandlesInput, RenderContext, SelectOption};

fn render_component(component: &mut impl Component, width: u16, rows: u16) -> TestTerminal {
    let ctx = RenderContext::new((width, rows));
    let lines = component.render(&ctx);
    let mut terminal = TestTerminal::new(width, rows);
    let mut screen = Screen::new();
    screen.render(&lines, width, &mut terminal).unwrap();
    terminal.flush().unwrap();
    terminal
}

fn render_component_with_screen(
    component: &mut impl Component,
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

fn render_lines(lines: &[wisp::tui::screen::Line], width: u16, rows: u16) -> TestTerminal {
    let mut terminal = TestTerminal::new(width, rows);
    let mut screen = Screen::new();
    screen.render(lines, width, &mut terminal).unwrap();
    terminal.flush().unwrap();
    terminal
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn sample_options() -> Vec<SelectOption> {
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

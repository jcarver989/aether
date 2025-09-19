use crate::components::{Input, Logo};
use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct ScreenProps {
    label: String,
    pub value: Option<State<String>>,
    has_focus: bool,
    multiline: bool,
}

#[component]
pub fn Screen(props: &ScreenProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let message = hooks.use_state(|| "".to_string());

    hooks.use_terminal_events(move |event| match event {
        TerminalEvent::Key(KeyEvent { code, kind, .. }) if kind != KeyEventKind::Release => {
            match code {
                KeyCode::Enter | KeyCode::Char(' ') => {}
                KeyCode::BackTab => {}
                KeyCode::Tab => {}
                _ => {}
            }
        }
        _ => {}
    });

    element! {
        View(flex_direction: FlexDirection::Column) {
            View(flex_direction: FlexDirection::Column) {
                Logo()
            }

            View {}

            View {
                Input(value: message)
            }
        }
    }
}

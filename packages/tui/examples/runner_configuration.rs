use tui::advanced::MouseCapture;
use tui::{App, AppEvent, Cursor, Frame, KeyCode, Line, ViewContext};

struct ConfiguredApp {
    exit_requested: bool,
}

impl App for ConfiguredApp {
    type Event = ();
    type Effect = ();
    type Error = std::io::Error;

    fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Option<Vec<()>> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                self.exit_requested = true;
                Some(vec![])
            }
            _ => Some(vec![]),
        }
    }

    fn view(&self, _ctx: &ViewContext) -> Frame {
        Frame::new(
            vec![
                Line::new("Runner configuration example"),
                Line::new("Shows how to customize Runner with builder methods."),
                Line::new("Press q to quit."),
            ],
            Cursor {
                row: 0,
                col: 0,
                is_visible: false,
            },
        )
    }

    fn should_exit(&self) -> bool {
        self.exit_requested
    }
}

/// This example demonstrates configuring the [`Runner`] with builder methods.
///
/// Use builder methods like `.mouse_capture()`, `.tick_rate()`, `.no_ticks()`,
/// and `.bracketed_paste()` to customize runtime behavior.
#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    use tui::Runner;

    Runner::new(ConfiguredApp {
        exit_requested: false,
    })
    .mouse_capture(MouseCapture::Disabled)
    .no_ticks()
    .run()
    .await
}

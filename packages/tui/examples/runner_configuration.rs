use tui::advanced::MouseCapture;
use tui::{App, AppEvent, Cursor, Frame, KeyCode, Line, Response, ViewContext};

struct ConfiguredApp;

impl App for ConfiguredApp {
    type Event = ();
    type Effect = ();
    type Error = std::io::Error;

    fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Response<()> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
            _ => Response::ok(),
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
}

/// This example demonstrates configuring the [`Runner`] with builder methods.
///
/// Use builder methods like `.mouse_capture()`, `.tick_rate()`, `.no_ticks()`,
/// and `.bracketed_paste()` to customize runtime behavior.
#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    use tui::Runner;

    Runner::new(ConfiguredApp)
        .mouse_capture(MouseCapture::Disabled)
        .no_ticks()
        .run()
        .await
}

use crossterm::event::{Event as CrosstermEvent, read};
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;

pub mod app;
pub use app::{App, AppEvent, Effects, Runner, run};

pub mod terminal;
pub use terminal::{MouseCapture, TerminalSession};

#[cfg(all(test, feature = "testing"))]
mod app_tests;

pub fn spawn_terminal_event_task() -> mpsc::UnboundedReceiver<CrosstermEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    spawn_blocking(move || {
        loop {
            let event = match read() {
                Ok(event) => event,
                Err(e) => {
                    eprintln!("Event read error: {e}");
                    continue;
                }
            };

            if tx.send(event).is_err() {
                break;
            }
        }
    });
    rx
}

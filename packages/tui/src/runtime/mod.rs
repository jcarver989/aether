use crossterm::event::{Event as CrosstermEvent, poll, read};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;

pub mod terminal;
pub use terminal::{MouseCapture, TerminalSession};

pub fn spawn_terminal_event_task() -> mpsc::UnboundedReceiver<CrosstermEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    spawn_blocking(move || {
        loop {
            if tx.is_closed() {
                break;
            }

            match poll(Duration::from_millis(50)).and_then(|ready| ready.then(read).transpose()) {
                Ok(Some(event)) => {
                    if tx.send(event).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(e) => eprintln!("Terminal event error: {e}"),
            }
        }
    });
    rx
}

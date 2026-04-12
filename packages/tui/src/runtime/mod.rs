use crossterm::event::{Event as CrosstermEvent, poll, read};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::{JoinHandle, spawn_blocking};
use tokio_util::sync::CancellationToken;

pub mod terminal;
pub mod terminal_runtime;
pub use terminal::{MouseCapture, TerminalSession};
pub use terminal_runtime::{TerminalConfig, TerminalRuntime};

pub(crate) struct EventTaskHandle {
    rx: mpsc::UnboundedReceiver<CrosstermEvent>,
    cancel: CancellationToken,
    join: JoinHandle<()>,
}

impl EventTaskHandle {
    pub(crate) fn rx(&mut self) -> &mut mpsc::UnboundedReceiver<CrosstermEvent> {
        &mut self.rx
    }

    pub(crate) async fn stop(self) {
        self.cancel.cancel();
        let _ = self.join.await;
    }
}

pub(crate) fn spawn_terminal_event_task() -> EventTaskHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let join = spawn_blocking(move || {
        loop {
            if task_cancel.is_cancelled() || tx.is_closed() {
                break;
            }

            match poll(Duration::from_millis(10)).and_then(|ready| ready.then(read).transpose()) {
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
    EventTaskHandle { rx, cancel, join }
}

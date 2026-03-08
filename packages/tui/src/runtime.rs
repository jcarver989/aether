use crossterm::event::{Event, read};
use tokio::{sync::mpsc, task::spawn_blocking};

pub fn spawn_terminal_event_task() -> mpsc::UnboundedReceiver<Event> {
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

use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub enum Event<I> {
    Input(I),
    Tick,
}

pub struct EventHandler {
    rx: mpsc::Receiver<Event<CrosstermEvent>>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel();
        
        thread::spawn(move || {
            loop {
                if crossterm::event::poll(tick_rate).unwrap() {
                    if let Ok(event) = crossterm::event::read() {
                        tx.send(Event::Input(event)).unwrap();
                    }
                } else {
                    tx.send(Event::Tick).unwrap();
                }
            }
        });

        EventHandler { rx }
    }

    pub fn next(&self) -> Result<Event<CrosstermEvent>, mpsc::RecvError> {
        self.rx.recv()
    }
}
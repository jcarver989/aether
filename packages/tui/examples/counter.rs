use tui::{App, AppEvent, Cursor, Frame, KeyCode, Line, Runner, ViewContext};

struct CounterApp {
    count: i32,
    exit_requested: bool,
}

impl App for CounterApp {
    type Event = ();
    type Effect = ();
    type Error = std::io::Error;

    fn update(
        &mut self,
        event: AppEvent<Self::Event>,
        _ctx: &ViewContext,
    ) -> Option<Vec<Self::Effect>> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                self.exit_requested = true;
                Some(vec![])
            }
            AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                self.count += 1;
                Some(vec![])
            }
            AppEvent::Key(key) if key.code == KeyCode::Char('k') => {
                self.count -= 1;
                Some(vec![])
            }
            _ => Some(vec![]),
        }
    }

    fn view(&self, _ctx: &ViewContext) -> Frame {
        Frame::new(
            vec![
                Line::new("Counter example"),
                Line::new(""),
                Line::new(format!("Count: {}", self.count)),
                Line::new("Press j/k to change the count."),
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

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    Runner::new(CounterApp {
        count: 0,
        exit_requested: false,
    })
    .run()
    .await
}

use tui::{App, AppEvent, Cursor, Response, Frame, KeyCode, Line, ViewContext, Runner};

struct CounterApp {
    count: i32,
}

impl App for CounterApp {
    type Event = ();
    type Effect = ();
    type Error = std::io::Error;

    fn update(
        &mut self,
        event: AppEvent<Self::Event>,
        _ctx: &ViewContext,
    ) -> Response<Self::Effect> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
            AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                self.count += 1;
                Response::ok()
            }
            AppEvent::Key(key) if key.code == KeyCode::Char('k') => {
                self.count -= 1;
                Response::ok()
            }
            _ => Response::ok(),
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
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    Runner::new(CounterApp { count: 0 }).run().await
}

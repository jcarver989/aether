use tui::{
    App, AppEvent, Cursor, Frame, KeyCode, Line, Response, ViewContext, Runner, Style,
    Widget, WidgetEvent,
};

struct IncrementButton {
    label: String,
}

impl Widget for IncrementButton {
    type Message = i32;

    fn on_event(&mut self, event: &WidgetEvent) -> Response<Self::Message> {
        let WidgetEvent::Key(key) = event else {
            return Response::ignored();
        };
        match key.code {
            KeyCode::Enter => Response::one(1),
            _ => Response::ignored(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        let style = Style::default().bold().color(context.theme.primary());
        vec![Line::with_style(format!("[ {} ]", self.label), style)]
    }
}

struct WidgetApp {
    count: i32,
    button: IncrementButton,
}

impl App for WidgetApp {
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
            AppEvent::Key(key) => {
                let result = self.button.on_event(&WidgetEvent::Key(key));
                for message in result.into_messages() {
                    self.count += message;
                }
                Response::ok()
            }
            _ => Response::ok(),
        }
    }

    fn view(&self, ctx: &ViewContext) -> Frame {
        let mut lines = vec![
            Line::new("Reusable widget example"),
            Line::new(""),
            Line::new(format!("Count: {}", self.count)),
            Line::new("Press Enter to activate the child widget."),
            Line::new("Press q to quit."),
            Line::new(""),
        ];
        lines.extend(self.button.render(ctx));

        Frame::new(
            lines,
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
    Runner::new(WidgetApp {
        count: 0,
        button: IncrementButton {
            label: "Increment".to_string(),
        },
    })
    .run()
    .await
}

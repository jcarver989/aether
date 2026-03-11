use tui::{
    App, AppEvent, Component, Cursor, Event, Frame, KeyCode, Line, Runner, Style, ViewContext,
};

struct IncrementButton {
    label: String,
}

impl Component for IncrementButton {
    type Message = i32;

    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(key) = event else {
            return None;
        };
        match key.code {
            KeyCode::Enter => Some(vec![1]),
            _ => None,
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
    exit_requested: bool,
}

impl App for WidgetApp {
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
            AppEvent::Key(key) => {
                let result = self.button.on_event(&Event::Key(key));
                for message in result.unwrap_or_default() {
                    self.count += message;
                }
                Some(vec![])
            }
            _ => Some(vec![]),
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

    fn should_exit(&self) -> bool {
        self.exit_requested
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    Runner::new(WidgetApp {
        count: 0,
        button: IncrementButton {
            label: "Increment".to_string(),
        },
        exit_requested: false,
    })
    .run()
    .await
}

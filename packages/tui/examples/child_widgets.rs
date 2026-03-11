use tui::{
    App, AppEvent, Cursor, Effects, Frame, KeyCode, Line, Outcome, ViewContext, Runner, Style,
    Widget, WidgetEvent,
};

struct IncrementButton {
    label: String,
}

impl Widget for IncrementButton {
    type Message = i32;

    fn on_event(&mut self, event: &WidgetEvent) -> Outcome<Self::Message> {
        let WidgetEvent::Key(key) = event else {
            return Outcome::ignored();
        };
        match key.code {
            KeyCode::Enter => Outcome::message(1),
            _ => Outcome::ignored(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        let style = if context.focused {
            Style::default().bold().color(context.theme.primary())
        } else {
            Style::default().color(context.theme.text_secondary())
        };

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
    ) -> Effects<Self::Effect> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => Effects::exit(),
            AppEvent::Key(key) => {
                let result = self.button.on_event(&WidgetEvent::Key(key));
                for message in result.into_messages() {
                    self.count += message;
                }
                Effects::none()
            }
            _ => Effects::none(),
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
        lines.extend(self.button.render(&ctx.with_focused(true)));

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

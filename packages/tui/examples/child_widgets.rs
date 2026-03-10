use tui::{
    App, AppEvent, Component, Cursor, Effects, Frame, InteractiveComponent, KeyCode, Line,
    MessageResult, RenderContext, Runner, Style, UiEvent,
};

struct IncrementButton {
    label: String,
}

impl Component for IncrementButton {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let style = if context.focused {
            Style::default().bold().color(context.theme.primary())
        } else {
            Style::default().color(context.theme.text_secondary())
        };

        vec![Line::with_style(format!("[ {} ]", self.label), style)]
    }
}

impl InteractiveComponent for IncrementButton {
    type Message = i32;

    fn on_event(&mut self, event: UiEvent) -> MessageResult<Self::Message> {
        match event {
            UiEvent::Key(key) if key.code == KeyCode::Enter => MessageResult::message(1),
            _ => MessageResult::ignored(),
        }
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
        _ctx: &RenderContext,
    ) -> Effects<Self::Effect> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => Effects::exit(),
            AppEvent::Key(key) => {
                let result = self.button.on_event(UiEvent::Key(key));
                for message in result.messages {
                    self.count += message;
                }
                Effects::none()
            }
            _ => Effects::none(),
        }
    }

    fn view(&self, ctx: &RenderContext) -> Frame {
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

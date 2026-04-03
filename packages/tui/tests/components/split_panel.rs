use super::*;
use crossterm::event::KeyCode;
use tui::{Cursor, Either, Frame, Line, SplitLayout, SplitPanel, Style};

struct StubComponent {
    label: String,
    messages: Vec<String>,
}

impl StubComponent {
    fn new(label: &str) -> Self {
        Self { label: label.into(), messages: Vec::new() }
    }

    fn with_messages(label: &str, msgs: Vec<&str>) -> Self {
        Self { label: label.into(), messages: msgs.into_iter().map(Into::into).collect() }
    }
}

impl Component for StubComponent {
    type Message = String;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<String>> {
        if let Event::Key(_) = event {
            if self.messages.is_empty() { Some(vec![]) } else { Some(self.messages.clone()) }
        } else {
            None
        }
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let mut lines = vec![Line::new(&self.label)];
        while lines.len() < ctx.size.height as usize {
            lines.push(Line::default());
        }
        Frame::new(lines)
    }
}

fn make_split() -> SplitPanel<StubComponent, StubComponent> {
    SplitPanel::new(StubComponent::new("LEFT"), StubComponent::new("RIGHT"), SplitLayout::fixed(15))
}

#[test]
fn renders_both_panels_side_by_side() {
    let mut split = make_split();
    let term = render_component(|ctx| split.render(ctx), 40, 3);
    assert_buffer_eq(&term, &["LEFT           RIGHT", "", ""]);
}

#[test]
fn renders_with_separator() {
    let mut split = make_split().with_separator("|", Style::default());
    let term = render_component(|ctx| split.render(ctx), 40, 3);
    assert_buffer_eq(&term, &["LEFT           |RIGHT", "               |", "               |"]);
}

#[test]
fn starts_with_left_focused() {
    let split = make_split();
    assert!(split.is_left_focused());
}

#[tokio::test]
async fn tab_switches_focus_to_right() {
    let mut split = make_split();
    assert!(split.is_left_focused());

    split.on_event(&Event::Key(key(KeyCode::Tab))).await;
    assert!(!split.is_left_focused());
}

#[tokio::test]
async fn backtab_switches_focus_to_left() {
    let mut split = make_split();
    split.focus_right();
    assert!(!split.is_left_focused());

    split.on_event(&Event::Key(key(KeyCode::BackTab))).await;
    assert!(split.is_left_focused());
}

#[tokio::test]
async fn routes_events_to_focused_child() {
    let mut split = SplitPanel::new(
        StubComponent::with_messages("L", vec!["from_left"]),
        StubComponent::with_messages("R", vec!["from_right"]),
        SplitLayout::fixed(10),
    );

    let result = split.on_event(&Event::Key(key(KeyCode::Char('a')))).await.unwrap();
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], Either::Left(s) if s == "from_left"));

    split.focus_right();
    let result = split.on_event(&Event::Key(key(KeyCode::Char('a')))).await.unwrap();
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], Either::Right(s) if s == "from_right"));
}

#[tokio::test]
async fn resize_keys_widen_left_panel() {
    let mut split = make_split().with_resize_keys();

    let term = render_component(|ctx| split.render(ctx), 40, 1);
    assert_buffer_eq(&term, &["LEFT           RIGHT"]);

    split.on_event(&Event::Key(key(KeyCode::Char('>')))).await;

    let term = render_component(|ctx| split.render(ctx), 40, 1);
    assert_buffer_eq(&term, &["LEFT               RIGHT"]);
}

#[tokio::test]
async fn resize_keys_narrow_left_panel() {
    let mut split = make_split().with_resize_keys();

    split.on_event(&Event::Key(key(KeyCode::Char('>')))).await;
    split.on_event(&Event::Key(key(KeyCode::Char('<')))).await;

    let term = render_component(|ctx| split.render(ctx), 40, 1);
    assert_buffer_eq(&term, &["LEFT           RIGHT"]);
}

#[tokio::test]
async fn resize_keys_disabled_by_default() {
    let mut split = SplitPanel::new(
        StubComponent::with_messages("L", vec!["got_it"]),
        StubComponent::new("R"),
        SplitLayout::fixed(10),
    );

    let result = split.on_event(&Event::Key(key(KeyCode::Char('>')))).await.unwrap();
    assert!(matches!(&result[0], Either::Left(s) if s == "got_it"));

    let term = render_component(|ctx| split.render(ctx), 40, 1);
    assert_buffer_eq(&term, &["L           R"]);
}

#[test]
fn cursor_from_right_panel_is_offset_by_left_width() {
    struct CursorComponent;
    impl Component for CursorComponent {
        type Message = ();
        async fn on_event(&mut self, _: &Event) -> Option<Vec<()>> {
            None
        }
        fn render(&mut self, _ctx: &ViewContext) -> Frame {
            Frame::new(vec![Line::new("input")]).with_cursor(Cursor::visible(0, 3))
        }
    }

    let mut split = SplitPanel::new(StubComponent::new("L"), CursorComponent, SplitLayout::fixed(15));
    split.focus_right();

    let ctx = ViewContext::new((40, 3));
    let frame = split.render(&ctx);
    let cursor = frame.cursor();
    assert!(cursor.is_visible);
    assert_eq!(cursor.row, 0);
    assert_eq!(cursor.col, 3 + 15);
}

#[test]
fn cursor_from_left_panel_is_not_offset() {
    struct CursorComponent;
    impl Component for CursorComponent {
        type Message = ();
        async fn on_event(&mut self, _: &Event) -> Option<Vec<()>> {
            None
        }
        fn render(&mut self, _ctx: &ViewContext) -> Frame {
            Frame::new(vec![Line::new("input")]).with_cursor(Cursor::visible(0, 5))
        }
    }

    let mut split = SplitPanel::new(CursorComponent, StubComponent::new("R"), SplitLayout::fixed(10));
    let ctx = ViewContext::new((40, 3));
    let frame = split.render(&ctx);
    let cursor = frame.cursor();
    assert!(cursor.is_visible);
    assert_eq!(cursor.col, 5);
}

#[tokio::test]
async fn focus_left_and_focus_right() {
    let mut split = make_split();
    assert!(split.is_left_focused());

    split.focus_right();
    assert!(!split.is_left_focused());

    split.focus_left();
    assert!(split.is_left_focused());
}

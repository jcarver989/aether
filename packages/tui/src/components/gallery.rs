use crossterm::event::KeyCode;
use crossterm::style::Color;

use super::component::{Component, Event};
use super::panel::Panel;
use super::wrap_selection;
use crate::focus::{FocusOutcome, FocusRing};
use crate::line::Line;
use crate::rendering::columns::side_by_side;
use crate::rendering::frame::{Cursor, Frame};
use crate::rendering::render_context::ViewContext;
use crate::style::Style;

const SIDEBAR_MIN: usize = 20;
const SIDEBAR_MAX: usize = 30;

pub enum GalleryMessage {
    Quit,
}

pub struct Gallery<T: Component> {
    entries: Vec<(String, T)>,
    selected: usize,
    focus: FocusRing,
}

impl<T: Component> Gallery<T> {
    pub fn new(entries: Vec<(String, T)>) -> Self {
        Self { entries, selected: 0, focus: FocusRing::new(2).without_wrap() }
    }
}

impl<T: Component> Component for Gallery<T> {
    type Message = GalleryMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        if let Event::Tick = event {
            if let Some((_, component)) = self.entries.get_mut(self.selected) {
                let _ = component.on_event(event).await;
            }
            return Some(vec![]);
        }

        let Event::Key(key) = event else {
            return None;
        };

        if key.code == KeyCode::Esc {
            return Some(vec![GalleryMessage::Quit]);
        }

        let outcome = self.focus.handle_key(*key);
        if matches!(outcome, FocusOutcome::FocusChanged) {
            return Some(vec![]);
        }

        if self.focus.is_focused(0) {
            match key.code {
                KeyCode::Up => {
                    wrap_selection(&mut self.selected, self.entries.len(), -1);
                    Some(vec![])
                }
                KeyCode::Down => {
                    wrap_selection(&mut self.selected, self.entries.len(), 1);
                    Some(vec![])
                }
                _ => Some(vec![]),
            }
        } else {
            if let Some((_, component)) = self.entries.get_mut(self.selected) {
                let _ = component.on_event(event).await;
            }
            Some(vec![])
        }
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        if self.entries.is_empty() {
            return Frame::new(vec![Line::new("No stories")]);
        }

        let sidebar_focused = self.focus.is_focused(0);
        let longest_name = self.entries.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
        let sidebar_width = (longest_name + 6).clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        let preview_width = (ctx.size.width as usize).saturating_sub(sidebar_width + 1);
        let height = ctx.size.height as usize;

        let sidebar_lines = render_sidebar(&self.entries, self.selected, sidebar_focused, sidebar_width, height, ctx);

        #[allow(clippy::cast_possible_truncation)]
        let preview_ctx = ctx.with_size((preview_width as u16, ctx.size.height));
        let (name, component) = &mut self.entries[self.selected];
        let name = name.clone();
        let preview_focused = self.focus.is_focused(1);
        let (preview_lines, story_cursor) = render_preview(&name, component, preview_focused, &preview_ctx);

        let sep_lines = prepend_separator(&preview_lines, ctx.theme.muted(), height);
        let merged = side_by_side(&sidebar_lines, &sep_lines, sidebar_width);

        let cursor = if preview_focused && story_cursor.is_visible {
            Cursor::visible(story_cursor.row + 2, story_cursor.col + sidebar_width + 1 + 2)
        } else {
            Cursor::hidden()
        };

        Frame::new(merged).with_cursor(cursor)
    }
}

fn render_sidebar<T: Component>(
    entries: &[(String, T)],
    selected: usize,
    focused: bool,
    width: usize,
    height: usize,
    ctx: &ViewContext,
) -> Vec<Line> {
    let mut lines = Vec::with_capacity(height);
    lines.push(Line::with_style(" Gallery", Style::fg(ctx.theme.accent()).bold()));
    lines.push(Line::default());

    for (i, (name, _)) in entries.iter().enumerate() {
        let is_selected = i == selected;
        let indicator = if is_selected { ">" } else { " " };
        let style = if is_selected && focused {
            ctx.theme.selected_row_style()
        } else if is_selected {
            Style::fg(ctx.theme.text_primary()).bold()
        } else {
            Style::fg(ctx.theme.text_secondary())
        };
        let mut line = Line::with_style(format!(" {indicator} {name}"), style);
        line.extend_bg_to_width(width);
        lines.push(line);
    }

    while lines.len() < height {
        lines.push(Line::default());
    }
    lines.truncate(height);

    lines
}

fn render_preview<T: Component>(
    name: &str,
    component: &mut T,
    focused: bool,
    ctx: &ViewContext,
) -> (Vec<Line>, Cursor) {
    let border_color = if focused { ctx.theme.accent() } else { ctx.theme.muted() };

    let inner_width = Panel::inner_width(ctx.size.width);
    let inner_ctx = ctx.with_size((inner_width, ctx.size.height.saturating_sub(4)));
    let frame = component.render(&inner_ctx);
    let (content_lines, cursor) = frame.into_parts();

    let footer = if focused { "[Shift+Tab] sidebar  [Esc] quit" } else { "[Tab] preview  [Esc] quit" };

    let mut panel = Panel::new(border_color).title(format!(" {name} ")).footer(footer);
    panel.push(content_lines);

    (panel.render(ctx), cursor)
}

fn prepend_separator(lines: &[Line], color: Color, height: usize) -> Vec<Line> {
    let mut result = Vec::with_capacity(height);
    for i in 0..height {
        let mut sep = Line::styled("│", color);
        if let Some(line) = lines.get(i) {
            sep.append_line(line);
        }
        result.push(sep);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyComponent {
        label: String,
    }

    impl Component for DummyComponent {
        type Message = ();

        async fn on_event(&mut self, _event: &Event) -> Option<Vec<()>> {
            None
        }

        fn render(&mut self, _ctx: &ViewContext) -> Frame {
            Frame::new(vec![Line::new(&self.label)])
        }
    }

    fn dummy(name: &str, label: &str) -> (String, DummyComponent) {
        (name.into(), DummyComponent { label: label.into() })
    }

    #[test]
    fn empty_gallery_renders_placeholder() {
        let mut gallery: Gallery<DummyComponent> = Gallery::new(vec![]);
        let ctx = ViewContext::new((80, 24));
        let frame = gallery.render(&ctx);
        assert_eq!(frame.lines()[0].plain_text(), "No stories");
    }

    #[test]
    fn sidebar_shows_all_entry_names() {
        let mut gallery = Gallery::new(vec![dummy("Alpha", "a"), dummy("Beta", "b")]);
        let ctx = ViewContext::new((80, 24));
        let frame = gallery.render(&ctx);
        let text: String = frame.lines().iter().map(|l| l.plain_text()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Alpha"), "should contain Alpha: {text}");
        assert!(text.contains("Beta"), "should contain Beta: {text}");
    }

    #[test]
    fn selected_entry_has_indicator() {
        let mut gallery = Gallery::new(vec![dummy("Alpha", "a"), dummy("Beta", "b")]);
        let ctx = ViewContext::new((80, 24));
        let frame = gallery.render(&ctx);
        let all_text: Vec<String> = frame.lines().iter().map(|l| l.plain_text()).collect();
        assert!(all_text.iter().any(|l| l.contains("> Alpha")), "should have > Alpha indicator: {all_text:?}");
    }

    #[tokio::test]
    async fn down_arrow_changes_selection() {
        let mut gallery = Gallery::new(vec![dummy("Alpha", "a"), dummy("Beta", "b")]);
        assert_eq!(gallery.selected, 0);

        let down = Event::Key(crossterm::event::KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE));
        gallery.on_event(&down).await;
        assert_eq!(gallery.selected, 1);
    }

    #[tokio::test]
    async fn esc_emits_quit() {
        let mut gallery = Gallery::new(vec![dummy("A", "a")]);
        let esc = Event::Key(crossterm::event::KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::NONE));
        let msgs = gallery.on_event(&esc).await.unwrap();
        assert!(matches!(msgs[0], GalleryMessage::Quit));
    }

    #[tokio::test]
    async fn tab_switches_focus() {
        let mut gallery = Gallery::new(vec![dummy("A", "a")]);
        assert!(gallery.focus.is_focused(0));

        let tab = Event::Key(crossterm::event::KeyEvent::new(KeyCode::Tab, crossterm::event::KeyModifiers::NONE));
        gallery.on_event(&tab).await;
        assert!(gallery.focus.is_focused(1));
    }
}

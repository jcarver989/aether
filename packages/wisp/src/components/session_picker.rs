use agent_client_protocol as acp;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use tui::{
    Combobox, Component, Cursor, Event, Frame, Line, MouseEventKind, PickerMessage, Searchable, Style, ViewContext,
    display_width_text, pad_text_to_width, truncate_text,
};

#[derive(Clone)]
pub struct SessionEntry(pub acp::SessionInfo);

impl Searchable for SessionEntry {
    fn search_text(&self) -> String {
        let SessionEntry(info) = self;
        let title = info.title.as_deref().unwrap_or("");
        let cwd = info.cwd.display();
        format!("{title} {cwd}")
    }
}

pub struct SessionPicker {
    combobox: Combobox<SessionEntry>,
}

pub enum SessionPickerMessage {
    Close,
    LoadSession { session_id: acp::SessionId, cwd: PathBuf },
}

impl SessionPicker {
    pub fn new(sessions: Vec<SessionEntry>) -> Self {
        Self { combobox: Combobox::new(sessions) }
    }
}

impl Component for SessionPicker {
    type Message = SessionPickerMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        if let Event::Mouse(mouse) = event {
            return match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.combobox.move_up();
                    Some(vec![])
                }
                MouseEventKind::ScrollDown => {
                    self.combobox.move_down();
                    Some(vec![])
                }
                _ => Some(vec![]),
            };
        }
        let msgs = self.combobox.handle_picker_event(event)?;
        let mapped = msgs
            .into_iter()
            .filter_map(|m| match m {
                PickerMessage::Close | PickerMessage::CloseAndPopChar => Some(SessionPickerMessage::Close),
                PickerMessage::Confirm(entry) => Some(SessionPickerMessage::LoadSession {
                    session_id: acp::SessionId::new(entry.0.session_id.0.to_string()),
                    cwd: entry.0.cwd,
                }),
                _ => None,
            })
            .collect();
        Some(mapped)
    }

    fn render(&mut self, context: &ViewContext) -> Frame {
        if self.combobox.is_empty() {
            return Frame::new(vec![Line::new(String::new()), Line::new("  No previous sessions found.")]);
        }

        let now = Utc::now();

        let mut lines = vec![Line::new(String::new())];
        let header = format!("  Resume a previous session: {}", self.combobox.query());
        lines.push(Line::new(&header));
        lines.push(Line::new(String::new()));

        let max_title_width = self
            .combobox
            .matches()
            .iter()
            .map(|e| {
                let title = display_title(&e.0);
                display_width_text(&title)
            })
            .max()
            .unwrap_or(0);

        let item_lines = self.combobox.render_items(context, |SessionEntry(info), is_selected, ctx| {
            let title = display_title(info);
            let relative = info.updated_at.as_deref().map(|ts| format_relative_time(ts, now)).unwrap_or_default();

            let padded_title = pad_text_to_width(&title, max_title_width);
            let line_text = format!("{padded_title}  {relative}");

            let max_width = ctx.size.width as usize;
            let truncated = truncate_text(&line_text, max_width);

            if is_selected {
                ctx.theme.selected_row_line(truncated)
            } else {
                let boundary = padded_title.len().min(truncated.len());
                let mut line = Line::new(&truncated[..boundary]);
                if truncated.len() > boundary {
                    line.push_with_style(&truncated[boundary..], Style::fg(ctx.theme.muted()));
                }
                line
            }
        });
        lines.extend(item_lines);
        let cursor = Cursor::visible(1, display_width_text(&header));
        Frame::new(lines).with_cursor(cursor)
    }
}

fn display_title(info: &acp::SessionInfo) -> String {
    info.title.clone().unwrap_or_else(|| {
        info.cwd.file_name().map_or_else(|| info.cwd.display().to_string(), |n| n.to_string_lossy().into_owned())
    })
}

pub fn format_relative_time(iso: &str, now: DateTime<Utc>) -> String {
    let Ok(ts) = iso.parse::<DateTime<Utc>>() else {
        return iso.to_string();
    };
    if ts.format("%Y").to_string() == now.format("%Y").to_string() {
        ts.format("%b %-d").to_string()
    } else {
        ts.format("%b %-d, %Y").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tui::testing::{assert_buffer_eq, render_component};
    use tui::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

    const W: u16 = 60;
    const H: u16 = 10;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn sample_sessions() -> Vec<SessionEntry> {
        vec![
            SessionEntry(
                acp::SessionInfo::new("sess-aaa-111", PathBuf::from("/home/user/project-a"))
                    .updated_at("2026-03-10T10:00:00Z".to_string())
                    .title("Fix the login page redirect bug".to_string()),
            ),
            SessionEntry(
                acp::SessionInfo::new("sess-bbb-222", PathBuf::from("/home/user/project-b"))
                    .updated_at("2026-03-09T10:00:00Z".to_string())
                    .title("Add unit tests for session store".to_string()),
            ),
        ]
    }

    fn expected_date(iso: &str) -> String {
        format_relative_time(iso, Utc::now())
    }

    #[test]
    fn empty_sessions_shows_message() {
        let mut picker = SessionPicker::new(vec![]);
        let term = render_component(|ctx| picker.render(ctx), W, H);
        assert_buffer_eq(&term, &["", "  No previous sessions found."]);
    }

    #[test]
    fn renders_titles_and_dates_with_first_selected() {
        let mut picker = SessionPicker::new(sample_sessions());
        let d1 = expected_date("2026-03-10T10:00:00Z");
        let d2 = expected_date("2026-03-09T10:00:00Z");
        let term = render_component(|ctx| picker.render(ctx), W, H);
        assert_buffer_eq(
            &term,
            &[
                "",
                "  Resume a previous session:",
                "",
                &format!("  Fix the login page redirect bug   {d1}"),
                &format!("  Add unit tests for session store  {d2}"),
            ],
        );
    }

    #[tokio::test]
    async fn navigation_moves_selection_down() {
        let mut picker = SessionPicker::new(sample_sessions());
        picker.on_event(&key(KeyCode::Down)).await;
        let d1 = expected_date("2026-03-10T10:00:00Z");
        let d2 = expected_date("2026-03-09T10:00:00Z");
        let term = render_component(|ctx| picker.render(ctx), W, H);
        assert_buffer_eq(
            &term,
            &[
                "",
                "  Resume a previous session:",
                "",
                &format!("  Fix the login page redirect bug   {d1}"),
                &format!("  Add unit tests for session store  {d2}"),
            ],
        );
    }

    #[tokio::test]
    async fn mouse_scroll_moves_selection() {
        let mut picker = SessionPicker::new(sample_sessions());
        assert_eq!(picker.combobox.selected_index(), 0);

        let scroll_down = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        let outcome = picker.on_event(&scroll_down).await;
        assert!(outcome.is_some(), "mouse scroll should be consumed");
        assert_eq!(picker.combobox.selected_index(), 1);

        let scroll_up = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        picker.on_event(&scroll_up).await;
        assert_eq!(picker.combobox.selected_index(), 0);
    }

    #[tokio::test]
    async fn query_displayed_in_header() {
        let mut picker = SessionPicker::new(sample_sessions());
        picker.on_event(&key(KeyCode::Char('f'))).await;
        picker.on_event(&key(KeyCode::Char('i'))).await;
        picker.on_event(&key(KeyCode::Char('x'))).await;

        let term = render_component(|ctx| picker.render(ctx), W, H);
        let lines = term.get_lines();
        let header = &lines[1];
        assert!(header.contains("fix"), "header should contain query text, got: {header}");
    }

    #[test]
    fn falls_back_to_cwd_basename_when_no_title() {
        let sessions = vec![SessionEntry(
            acp::SessionInfo::new("sess-ccc-333", PathBuf::from("/home/user/my-project"))
                .updated_at("2026-03-10T10:00:00Z".to_string()),
        )];
        let mut picker = SessionPicker::new(sessions);
        let d = expected_date("2026-03-10T10:00:00Z");
        let term = render_component(|ctx| picker.render(ctx), W, H);
        assert_buffer_eq(&term, &["", "  Resume a previous session:", "", &format!("  my-project  {d}")]);
    }
}

use crate::tui::{
    Combobox, Component, Event, Line, PickerKey, PickerMessage, Searchable, Style, ViewContext,
    classify_key, display_width_text, pad_text_to_width, truncate_text,
};
use agent_client_protocol as acp;

#[derive(Clone)]
pub struct SessionEntry(pub acp::SessionInfo);

impl Searchable for SessionEntry {
    fn search_text(&self) -> String {
        let title = self.0.title.as_deref().unwrap_or("");
        let cwd = self.0.cwd.display();
        let id = self.0.session_id.0.as_ref();
        format!("{title} {cwd} {id}")
    }
}

pub struct SessionPicker {
    combobox: Combobox<SessionEntry>,
}

pub type SessionPickerMessage = PickerMessage<SessionEntry>;

impl SessionPicker {
    pub fn new(sessions: Vec<SessionEntry>) -> Self {
        Self {
            combobox: Combobox::new(sessions),
        }
    }
}

impl Component for SessionPicker {
    type Message = SessionPickerMessage;

    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(key_event) = event else {
            return None;
        };
        match classify_key(*key_event, self.combobox.query().is_empty()) {
            PickerKey::Escape | PickerKey::BackspaceOnEmpty => Some(vec![PickerMessage::Close]),
            PickerKey::MoveUp => {
                self.combobox.move_up();
                Some(vec![])
            }
            PickerKey::MoveDown => {
                self.combobox.move_down();
                Some(vec![])
            }
            PickerKey::Confirm => {
                if let Some(entry) = self.combobox.selected().cloned() {
                    Some(vec![PickerMessage::Confirm(entry)])
                } else {
                    Some(vec![PickerMessage::Close])
                }
            }
            PickerKey::Char(c) => {
                self.combobox.push_query_char(c);
                Some(vec![])
            }
            PickerKey::Backspace => {
                self.combobox.pop_query_char();
                Some(vec![])
            }
            PickerKey::MoveLeft
            | PickerKey::MoveRight
            | PickerKey::ControlChar
            | PickerKey::Other => Some(vec![]),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        if self.combobox.is_empty() {
            return vec![
                Line::new(String::new()),
                Line::new("  No previous sessions found."),
            ];
        }

        let mut lines = vec![Line::new(String::new())];
        lines.push(Line::new("  Resume a previous session:"));
        lines.push(Line::new(String::new()));

        let max_cwd_width = self
            .combobox
            .matches()
            .iter()
            .map(|e| display_width_text(&format!("  {}", e.0.cwd.display())))
            .max()
            .unwrap_or(0);

        let item_lines = self
            .combobox
            .render_items(context, |entry, is_selected, ctx| {
                let prefix = if is_selected { "▶ " } else { "  " };
                let cwd = entry.0.cwd.display().to_string();
                let time = entry.0.updated_at.as_deref().unwrap_or("");
                let id_short = truncate_session_id(entry.0.session_id.0.as_ref());

                let cwd_part = format!("{prefix}{cwd}");
                let padded_cwd = pad_text_to_width(&cwd_part, max_cwd_width);
                let line_text = format!("{padded_cwd}  {time}  {id_short}");

                let max_width = ctx.size.width as usize;
                let truncated = truncate_text(&line_text, max_width);

                if is_selected {
                    let mut line = Line::with_style(truncated, ctx.theme.selected_row_style());
                    line.extend_bg_to_width(max_width);
                    line
                } else {
                    let mut line = Line::new(&truncated[..padded_cwd.len().min(truncated.len())]);
                    if truncated.len() > padded_cwd.len() {
                        line.push_with_style(
                            &truncated[padded_cwd.len()..],
                            Style::fg(ctx.theme.muted()),
                        );
                    }
                    line
                }
            });
        lines.extend(item_lines);
        lines
    }
}

fn truncate_session_id(id: &str) -> String {
    if id.len() <= 8 {
        id.to_string()
    } else {
        format!("{}…", &id[..8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_picker::rendered_lines_from;
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn sample_sessions() -> Vec<SessionEntry> {
        vec![
            SessionEntry(
                acp::SessionInfo::new("sess-aaa-111", PathBuf::from("/home/user/project-a"))
                    .updated_at("2026-03-10T10:00:00Z".to_string()),
            ),
            SessionEntry(
                acp::SessionInfo::new("sess-bbb-222", PathBuf::from("/home/user/project-b"))
                    .updated_at("2026-03-09T10:00:00Z".to_string()),
            ),
        ]
    }

    #[test]
    fn empty_sessions_shows_message() {
        let picker = SessionPicker::new(vec![]);
        let context = ViewContext::new((120, 40));
        let lines = rendered_lines_from(&picker.render(&context));
        assert!(lines.iter().any(|l| l.contains("No previous sessions")));
    }

    #[test]
    fn renders_all_sessions() {
        let picker = SessionPicker::new(sample_sessions());
        let context = ViewContext::new((120, 40));
        let lines = rendered_lines_from(&picker.render(&context));
        assert!(lines.iter().any(|l| l.contains("project-a")));
        assert!(lines.iter().any(|l| l.contains("project-b")));
    }

    #[test]
    fn confirm_returns_selected() {
        let mut picker = SessionPicker::new(sample_sessions());
        let outcome = picker.on_event(&key(KeyCode::Enter));
        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PickerMessage::Confirm(_)]
        ));
    }

    #[test]
    fn escape_closes() {
        let mut picker = SessionPicker::new(sample_sessions());
        let outcome = picker.on_event(&key(KeyCode::Esc));
        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PickerMessage::Close]
        ));
    }

    #[test]
    fn navigation_changes_selection() {
        let mut picker = SessionPicker::new(sample_sessions());
        let context = ViewContext::new((120, 40));

        let lines_before = rendered_lines_from(&picker.render(&context));
        let selected_before = lines_before.iter().find(|l| l.starts_with('▶')).cloned();

        picker.on_event(&key(KeyCode::Down));

        let lines_after = rendered_lines_from(&picker.render(&context));
        let selected_after = lines_after.iter().find(|l| l.starts_with('▶')).cloned();

        assert_ne!(selected_before, selected_after);
    }

    #[test]
    fn truncate_session_id_shortens_long_ids() {
        assert_eq!(truncate_session_id("abcdefgh-1234-5678"), "abcdefgh…");
        assert_eq!(truncate_session_id("short"), "short");
    }
}

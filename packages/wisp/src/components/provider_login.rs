use crate::components::wrap_selection;
use crate::tui::KeyCode;
use crate::tui::{Line, Response, ViewContext, Widget, WidgetEvent};

pub struct ProviderLoginOverlay {
    pub entries: Vec<ProviderLoginEntry>,
    pub selected_index: usize,
}

pub struct ProviderLoginEntry {
    pub method_id: String,
    pub name: String,
    pub status: ProviderLoginStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderLoginStatus {
    NeedsLogin,
    Authenticating,
}

pub enum ProviderLoginMessage {
    Close,
    Authenticate(String),
}

impl Widget for ProviderLoginOverlay {
    type Message = ProviderLoginMessage;

    fn on_event(&mut self, event: &WidgetEvent) -> Response<Self::Message> {
        let WidgetEvent::Key(key) = event else {
            return Response::ignored();
        };
        match key.code {
            KeyCode::Esc => Response::one(ProviderLoginMessage::Close),
            KeyCode::Up => {
                self.move_selection_up();
                Response::ok()
            }
            KeyCode::Down => {
                self.move_selection_down();
                Response::ok()
            }
            KeyCode::Enter => {
                if let Some(entry) = self
                    .entries
                    .get(self.selected_index)
                    .filter(|e| e.status == ProviderLoginStatus::NeedsLogin)
                {
                    return Response::one(ProviderLoginMessage::Authenticate(
                        entry.method_id.clone(),
                    ));
                }
                Response::ok()
            }
            _ => Response::ok(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        if self.entries.is_empty() {
            return vec![Line::new("  (no providers need login)".to_string())];
        }

        self.entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let selected = i == self.selected_index;
                let prefix = if selected { "▶ " } else { "  " };
                let (indicator, detail) = match &entry.status {
                    ProviderLoginStatus::NeedsLogin => ("⚡", "needs login"),
                    ProviderLoginStatus::Authenticating => ("⏳", "authenticating..."),
                };
                let text = format!("{prefix}{}  {indicator} {detail}", entry.name);
                if selected {
                    Line::with_style(
                        text,
                        context
                            .theme
                            .selected_row_style_with_fg(context.theme.warning()),
                    )
                } else {
                    Line::styled(text, context.theme.warning())
                }
            })
            .collect()
    }
}

pub fn provider_login_summary(entries: &[ProviderLoginEntry]) -> String {
    if entries.is_empty() {
        return "all logged in".to_string();
    }
    let needs_login = entries
        .iter()
        .filter(|e| e.status == ProviderLoginStatus::NeedsLogin)
        .count();
    let authenticating = entries
        .iter()
        .filter(|e| e.status == ProviderLoginStatus::Authenticating)
        .count();
    [
        (needs_login, "needs login"),
        (authenticating, "authenticating"),
    ]
    .iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, label)| format!("{count} {label}"))
    .collect::<Vec<_>>()
    .join(", ")
}

impl ProviderLoginOverlay {
    pub fn new(entries: Vec<ProviderLoginEntry>) -> Self {
        Self {
            entries,
            selected_index: 0,
        }
    }

    fn move_selection_up(&mut self) {
        wrap_selection(&mut self.selected_index, self.entries.len(), -1);
    }

    fn move_selection_down(&mut self) {
        wrap_selection(&mut self.selected_index, self.entries.len(), 1);
    }

    pub fn set_authenticating(&mut self, method_id: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.method_id == method_id) {
            entry.status = ProviderLoginStatus::Authenticating;
        }
    }

    pub fn remove_entry(&mut self, method_id: &str) {
        self.entries.retain(|e| e.method_id != method_id);
        self.selected_index = self
            .selected_index
            .min(self.entries.len().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{KeyEvent, KeyModifiers};

    fn sample_entries() -> Vec<ProviderLoginEntry> {
        vec![ProviderLoginEntry {
            method_id: "codex".to_string(),
            name: "Codex".to_string(),
            status: ProviderLoginStatus::NeedsLogin,
        }]
    }

    #[test]
    fn renders_entries_with_status_indicators() {
        let overlay = ProviderLoginOverlay::new(sample_entries());
        let ctx = ViewContext::new((80, 24));
        let lines = overlay.render(&ctx);

        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("Codex"), "should contain provider name");
        assert!(text.contains("⚡"), "needs login should show bolt");
    }

    #[test]
    fn enter_on_needs_login_emits_authenticate() {
        let mut overlay = ProviderLoginOverlay::new(sample_entries());
        let outcome = overlay.on_event(&WidgetEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        let messages = outcome.into_messages();
        match messages.as_slice() {
            [ProviderLoginMessage::Authenticate(id)] => assert_eq!(id, "codex"),
            _ => panic!("Expected Authenticate message"),
        }
    }

    #[test]
    fn enter_on_authenticating_is_noop() {
        let mut entries = sample_entries();
        entries[0].status = ProviderLoginStatus::Authenticating;
        let mut overlay = ProviderLoginOverlay::new(entries);
        let outcome = overlay.on_event(&WidgetEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert!(outcome.into_messages().is_empty());
    }

    #[test]
    fn esc_closes_overlay() {
        let mut overlay = ProviderLoginOverlay::new(sample_entries());
        let outcome = overlay.on_event(&WidgetEvent::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        let messages = outcome.into_messages();
        assert!(matches!(
            messages.as_slice(),
            [ProviderLoginMessage::Close]
        ));
    }

    #[test]
    fn empty_entries_shows_placeholder() {
        let overlay = ProviderLoginOverlay::new(vec![]);
        let ctx = ViewContext::new((80, 24));
        let lines = overlay.render(&ctx);
        assert!(lines[0].plain_text().contains("no providers need login"));
    }

    #[test]
    fn set_authenticating_updates_status() {
        let mut overlay = ProviderLoginOverlay::new(sample_entries());
        overlay.set_authenticating("codex");
        assert_eq!(
            overlay.entries[0].status,
            ProviderLoginStatus::Authenticating
        );
    }

    #[test]
    fn remove_entry_clamps_selection() {
        let entries = vec![
            ProviderLoginEntry {
                method_id: "a".to_string(),
                name: "A".to_string(),
                status: ProviderLoginStatus::NeedsLogin,
            },
            ProviderLoginEntry {
                method_id: "b".to_string(),
                name: "B".to_string(),
                status: ProviderLoginStatus::NeedsLogin,
            },
        ];
        let mut overlay = ProviderLoginOverlay::new(entries);
        overlay.selected_index = 1;
        overlay.remove_entry("b");
        assert_eq!(overlay.entries.len(), 1);
        assert_eq!(overlay.selected_index, 0);
    }

    #[test]
    fn provider_login_summary_formats_correctly() {
        assert_eq!(provider_login_summary(&[]), "all logged in");
        assert_eq!(provider_login_summary(&sample_entries()), "1 needs login");
    }
}

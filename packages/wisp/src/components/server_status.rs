use crate::components::wrap_selection;
use crate::tui::KeyCode;
use crate::tui::{Line, Outcome, ViewContext, Widget, WidgetEvent};
use acp_utils::notifications::{McpServerStatus, McpServerStatusEntry};

pub struct ServerStatusOverlay {
    pub entries: Vec<McpServerStatusEntry>,
    pub selected_index: usize,
}

pub enum ServerStatusMessage {
    Close,
    Authenticate(String),
}

impl Widget for ServerStatusOverlay {
    type Message = ServerStatusMessage;

    fn on_event(&mut self, event: &WidgetEvent) -> Outcome<Self::Message> {
        let WidgetEvent::Key(key) = event else {
            return Outcome::ignored();
        };
        match key.code {
            KeyCode::Esc => Outcome::message(ServerStatusMessage::Close),
            KeyCode::Up => {
                self.move_selection_up();
                Outcome::consumed()
            }
            KeyCode::Down => {
                self.move_selection_down();
                Outcome::consumed()
            }
            KeyCode::Enter => {
                if let Some(entry) = self
                    .entries
                    .get(self.selected_index)
                    .filter(|e| matches!(e.status, McpServerStatus::NeedsOAuth))
                {
                    return Outcome::message(ServerStatusMessage::Authenticate(
                        entry.name.clone(),
                    ));
                }
                Outcome::consumed()
            }
            _ => Outcome::consumed(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        if self.entries.is_empty() {
            return vec![Line::new("  (no MCP servers configured)".to_string())];
        }

        self.entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let selected = i == self.selected_index;
                let prefix = if selected { "▶ " } else { "  " };
                let (indicator, detail) = match &entry.status {
                    McpServerStatus::Connected { tool_count } => {
                        ("✓", format!("{tool_count} tools"))
                    }
                    McpServerStatus::Failed { error } => ("✗", error.clone()),
                    McpServerStatus::NeedsOAuth => ("⚡", "needs authentication".to_string()),
                };
                let text = format!("{prefix}{}  {indicator} {detail}", entry.name);
                match &entry.status {
                    McpServerStatus::Connected { .. } => {
                        if selected {
                            Line::with_style(text, context.theme.selected_row_style())
                        } else {
                            Line::new(text)
                        }
                    }
                    McpServerStatus::Failed { .. } => {
                        if selected {
                            Line::with_style(
                                text,
                                context
                                    .theme
                                    .selected_row_style_with_fg(context.theme.error()),
                            )
                        } else {
                            Line::styled(text, context.theme.error())
                        }
                    }
                    McpServerStatus::NeedsOAuth => {
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
                    }
                }
            })
            .collect()
    }
}

pub fn server_status_summary(statuses: &[McpServerStatusEntry]) -> String {
    if statuses.is_empty() {
        return "none".to_string();
    }
    let (mut c, mut n, mut f) = (0usize, 0usize, 0usize);
    for s in statuses {
        match &s.status {
            McpServerStatus::Connected { .. } => c += 1,
            McpServerStatus::NeedsOAuth => n += 1,
            McpServerStatus::Failed { .. } => f += 1,
        }
    }
    [(c, "connected"), (n, "needs auth"), (f, "failed")]
        .iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl ServerStatusOverlay {
    pub fn new(entries: Vec<McpServerStatusEntry>) -> Self {
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

    pub fn update_entries(&mut self, entries: Vec<McpServerStatusEntry>) {
        let prev_index = self.selected_index;
        self.entries = entries;
        self.selected_index = prev_index.min(self.entries.len().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<McpServerStatusEntry> {
        vec![
            McpServerStatusEntry {
                name: "github".to_string(),
                status: McpServerStatus::Connected { tool_count: 5 },
            },
            McpServerStatusEntry {
                name: "linear".to_string(),
                status: McpServerStatus::NeedsOAuth,
            },
            McpServerStatusEntry {
                name: "slack".to_string(),
                status: McpServerStatus::Failed {
                    error: "connection timeout".to_string(),
                },
            },
        ]
    }

    #[test]
    fn renders_all_entries_with_status_indicators() {
        let overlay = ServerStatusOverlay::new(sample_entries());
        let ctx = ViewContext::new((80, 24));
        let lines = overlay.render(&ctx);

        assert_eq!(lines.len(), 3);
        let text0 = lines[0].plain_text();
        assert!(text0.contains("github"), "should contain server name");
        assert!(text0.contains("✓"), "connected should show checkmark");
        assert!(text0.contains("5 tools"), "should show tool count");

        let text1 = lines[1].plain_text();
        assert!(text1.contains("linear"), "should contain server name");
        assert!(text1.contains("⚡"), "needs auth should show bolt");

        let text2 = lines[2].plain_text();
        assert!(text2.contains("slack"), "should contain server name");
        assert!(text2.contains("✗"), "failed should show X");
        assert!(text2.contains("connection timeout"), "should show error");
    }

    #[test]
    fn selected_entry_has_pointer() {
        let overlay = ServerStatusOverlay::new(sample_entries());
        let ctx = ViewContext::new((80, 24));
        let lines = overlay.render(&ctx);

        assert!(lines[0].plain_text().starts_with("▶"));
        assert!(lines[1].plain_text().starts_with("  "));
    }

    #[test]
    fn navigation_wraps_around() {
        let mut overlay = ServerStatusOverlay::new(sample_entries());

        overlay.move_selection_up();
        assert_eq!(overlay.selected_index, 2);

        overlay.move_selection_down();
        assert_eq!(overlay.selected_index, 0);
    }

    #[test]
    fn enter_on_needs_oauth_emits_authenticate() {
        let mut overlay = ServerStatusOverlay::new(sample_entries());
        overlay.selected_index = 1; // linear - NeedsOAuth

        let outcome = overlay.on_event(&WidgetEvent::Key(crate::tui::KeyEvent::new(
            KeyCode::Enter,
            crate::tui::KeyModifiers::NONE,
        )));
        let messages = outcome.into_messages();
        match messages.as_slice() {
            [ServerStatusMessage::Authenticate(name)] => assert_eq!(name, "linear"),
            _ => panic!("Expected Authenticate message"),
        }
    }

    #[test]
    fn enter_on_connected_is_noop() {
        let mut overlay = ServerStatusOverlay::new(sample_entries());
        overlay.selected_index = 0; // github - Connected

        let outcome = overlay.on_event(&WidgetEvent::Key(crate::tui::KeyEvent::new(
            KeyCode::Enter,
            crate::tui::KeyModifiers::NONE,
        )));
        assert!(outcome.into_messages().is_empty());
    }

    #[test]
    fn esc_closes_overlay() {
        let mut overlay = ServerStatusOverlay::new(sample_entries());
        let outcome = overlay.on_event(&WidgetEvent::Key(crate::tui::KeyEvent::new(
            KeyCode::Esc,
            crate::tui::KeyModifiers::NONE,
        )));
        let messages = outcome.into_messages();
        assert!(matches!(
            messages.as_slice(),
            [ServerStatusMessage::Close]
        ));
    }

    #[test]
    fn empty_entries_shows_placeholder() {
        let overlay = ServerStatusOverlay::new(vec![]);
        let ctx = ViewContext::new((80, 24));
        let lines = overlay.render(&ctx);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("no MCP servers configured"));
    }

    #[test]
    fn update_entries_clamps_index() {
        let mut overlay = ServerStatusOverlay::new(sample_entries());
        overlay.selected_index = 2;

        overlay.update_entries(vec![McpServerStatusEntry {
            name: "github".to_string(),
            status: McpServerStatus::Connected { tool_count: 3 },
        }]);
        assert_eq!(overlay.selected_index, 0);
    }
}

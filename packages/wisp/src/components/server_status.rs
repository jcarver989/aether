use crate::components::wrap_selection;
use crate::tui::{Component, HandlesInput, InputOutcome, Line, RenderContext, Style};
use acp_utils::notifications::{McpServerStatus, McpServerStatusEntry};
use crossterm::event::{KeyCode, KeyEvent};

pub struct ServerStatusOverlay {
    pub entries: Vec<McpServerStatusEntry>,
    pub selected_index: usize,
}

pub enum ServerStatusAction {
    Close,
    Authenticate(String),
}

impl Component for ServerStatusOverlay {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
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
                            Line::with_style(
                                text,
                                Style::fg(context.theme.text_primary)
                                    .bg_color(context.theme.highlight_bg),
                            )
                        } else {
                            Line::new(text)
                        }
                    }
                    McpServerStatus::Failed { .. } => {
                        if selected {
                            Line::with_style(
                                text,
                                Style::fg(context.theme.error).bg_color(context.theme.highlight_bg),
                            )
                        } else {
                            Line::styled(text, context.theme.error)
                        }
                    }
                    McpServerStatus::NeedsOAuth => {
                        if selected {
                            Line::with_style(
                                text,
                                Style::fg(context.theme.warning)
                                    .bg_color(context.theme.highlight_bg),
                            )
                        } else {
                            Line::styled(text, context.theme.warning)
                        }
                    }
                }
            })
            .collect()
    }
}

impl HandlesInput for ServerStatusOverlay {
    type Action = ServerStatusAction;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        match key_event.code {
            KeyCode::Esc => InputOutcome::action_and_render(ServerStatusAction::Close),
            KeyCode::Up => {
                self.move_selection_up();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Down => {
                self.move_selection_down();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Enter => {
                if let Some(entry) = self
                    .entries
                    .get(self.selected_index)
                    .filter(|e| matches!(e.status, McpServerStatus::NeedsOAuth))
                {
                    return InputOutcome::action_and_render(ServerStatusAction::Authenticate(
                        entry.name.clone(),
                    ));
                }
                InputOutcome::consumed()
            }
            _ => InputOutcome::consumed(),
        }
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
        let mut overlay = ServerStatusOverlay::new(sample_entries());
        let ctx = RenderContext::new((80, 24));
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
        let mut overlay = ServerStatusOverlay::new(sample_entries());
        let ctx = RenderContext::new((80, 24));
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

        let outcome = overlay.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        match outcome.action {
            Some(ServerStatusAction::Authenticate(name)) => assert_eq!(name, "linear"),
            _ => panic!("Expected Authenticate action"),
        }
    }

    #[test]
    fn enter_on_connected_is_noop() {
        let mut overlay = ServerStatusOverlay::new(sample_entries());
        overlay.selected_index = 0; // github - Connected

        let outcome = overlay.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(outcome.action.is_none());
    }

    #[test]
    fn esc_closes_overlay() {
        let mut overlay = ServerStatusOverlay::new(sample_entries());
        let outcome = overlay.handle_key(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(matches!(outcome.action, Some(ServerStatusAction::Close)));
    }

    #[test]
    fn empty_entries_shows_placeholder() {
        let mut overlay = ServerStatusOverlay::new(vec![]);
        let ctx = RenderContext::new((80, 24));
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

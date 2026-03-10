use crate::components::{Component, InteractiveComponent, MessageResult, RenderContext, UiEvent};
use crate::focus::{FocusGroup, NavigationResult};
use crate::line::Line;
use crate::style::Style;
use crossterm::event::KeyCode;

/// Messages emitted by a [`Dialog`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogMessage {
    /// User confirmed the dialog (Enter or clicked confirm button).
    Confirm,
    /// User cancelled the dialog (Esc or clicked cancel button).
    Cancel,
}

/// A simple confirmation dialog with focusable buttons.
///
/// The dialog renders a message with Confirm/Cancel buttons and handles
/// keyboard navigation between them. It demonstrates the intended focus model:
/// - Tab/BackTab navigate between buttons
/// - Enter confirms the focused action
/// - Esc always cancels
///
/// # Example
///
/// ```rust
/// use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
/// use tui::{Dialog, DialogMessage, InteractiveComponent, UiEvent};
///
/// let mut dialog = Dialog::new("Save changes?".to_string())
///     .with_confirm_label("Save")
///     .with_cancel_label("Discard");
///
/// let event = UiEvent::Key(KeyEvent {
///     code: KeyCode::Enter,
///     modifiers: KeyModifiers::empty(),
///     kind: KeyEventKind::Press,
///     state: KeyEventState::empty(),
/// });
///
/// let result = dialog.on_event(event);
/// if let Some(DialogMessage::Confirm) = result.messages.first() {
///     // Handle save
/// }
/// ```
pub struct Dialog {
    message: String,
    confirm_label: String,
    cancel_label: String,
    focus: FocusGroup,
}

impl Dialog {
    /// Create a new dialog with the given message.
    ///
    /// Focus starts on the Confirm button (index 0).
    /// Cancel is at index 1.
    pub fn new(message: String) -> Self {
        Self {
            message,
            confirm_label: "Confirm".to_string(),
            cancel_label: "Cancel".to_string(),
            focus: FocusGroup::new(2), // Not a scope - wrap between buttons
        }
    }

    /// Set a custom label for the confirm button.
    pub fn with_confirm_label(mut self, label: impl Into<String>) -> Self {
        self.confirm_label = label.into();
        self
    }

    /// Set a custom label for the cancel button.
    pub fn with_cancel_label(mut self, label: impl Into<String>) -> Self {
        self.cancel_label = label.into();
        self
    }

    /// Returns `true` if the confirm button is focused.
    pub fn is_confirm_focused(&self) -> bool {
        self.focus.is_focused(0)
    }

    /// Returns `true` if the cancel button is focused.
    pub fn is_cancel_focused(&self) -> bool {
        self.focus.is_focused(1)
    }

    /// Focus the confirm button.
    pub fn focus_confirm(&mut self) {
        self.focus.focus(0);
    }

    /// Focus the cancel button.
    pub fn focus_cancel(&mut self) {
        self.focus.focus(1);
    }
}

impl Component for Dialog {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let width = context.size.width as usize;
        let content_width = width.saturating_sub(4); // 2 for borders, 2 for padding

        let mut lines = Vec::new();

        // Top border
        let top_border = format!("┌{}┐", "─".repeat(width.saturating_sub(2)));
        lines.push(Line::styled(top_border, context.theme.primary()));

        // Empty line
        lines.push(Line::styled(
            format!("│{}│", " ".repeat(content_width)),
            context.theme.primary(),
        ));

        // Message line (centered if shorter than width)
        let msg_padding = content_width.saturating_sub(self.message.len());
        let left_pad = msg_padding / 2;
        let right_pad = msg_padding - left_pad;
        let msg_line = format!(
            "│ {}{}{} │",
            " ".repeat(left_pad),
            self.message,
            " ".repeat(right_pad)
        );
        lines.push(Line::styled(msg_line, context.theme.text_primary()));

        // Empty line
        lines.push(Line::styled(
            format!("│{}│", " ".repeat(content_width)),
            context.theme.primary(),
        ));

        // Buttons line
        let confirm_style = if self.is_confirm_focused() {
            Style::default().color(context.theme.primary()).bold()
        } else {
            Style::default().color(context.theme.text_secondary())
        };
        let cancel_style = if self.is_cancel_focused() {
            Style::default().color(context.theme.primary()).bold()
        } else {
            Style::default().color(context.theme.text_secondary())
        };

        let confirm_prefix = if self.is_confirm_focused() {
            "▶ "
        } else {
            "  "
        };
        let cancel_prefix = if self.is_cancel_focused() {
            "▶ "
        } else {
            "  "
        };

        let confirm_text = format!("[{confirm_prefix}{}]", self.confirm_label);
        let cancel_text = format!("[{cancel_prefix}{}]", self.cancel_label);

        let button_spacing = 4;
        let buttons_total = confirm_text.len() + cancel_text.len() + button_spacing;
        let btn_padding = content_width.saturating_sub(buttons_total);
        let btn_left_pad = btn_padding / 2;
        let btn_right_pad = btn_padding - btn_left_pad;

        let mut buttons_line = Line::styled(
            format!("│ {}", " ".repeat(btn_left_pad)),
            context.theme.primary(),
        );
        buttons_line.push_with_style(confirm_text, confirm_style);
        buttons_line.push_with_style(
            format!("{}{}", " ".repeat(button_spacing), cancel_text),
            cancel_style,
        );
        buttons_line.push_with_style(
            format!("{}│", " ".repeat(btn_right_pad)),
            Style::default().color(context.theme.primary()),
        );
        lines.push(buttons_line);

        // Empty line
        lines.push(Line::styled(
            format!("│{}│", " ".repeat(content_width)),
            context.theme.primary(),
        ));

        // Bottom border
        let bottom_border = format!("└{}┘", "─".repeat(width.saturating_sub(2)));
        lines.push(Line::styled(bottom_border, context.theme.primary()));

        lines
    }
}

impl InteractiveComponent for Dialog {
    type Message = DialogMessage;

    fn on_event(&mut self, event: UiEvent) -> MessageResult<Self::Message> {
        match &event {
            UiEvent::Key(key_event) => {
                match key_event.code {
                    KeyCode::Esc => return MessageResult::message(DialogMessage::Cancel),
                    KeyCode::Enter => {
                        // Confirm on Enter, regardless of focus
                        return MessageResult::message(DialogMessage::Confirm);
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        if matches!(self.focus.navigation(&event), NavigationResult::Moved) {
                            return MessageResult::consumed();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        MessageResult::ignored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn test_context() -> RenderContext {
        RenderContext::new((40, 10))
    }

    #[test]
    fn new_dialog_starts_with_confirm_focused() {
        let dialog = Dialog::new("Test".to_string());
        assert!(dialog.is_confirm_focused());
        assert!(!dialog.is_cancel_focused());
    }

    #[test]
    fn tab_moves_to_cancel() {
        let mut dialog = Dialog::new("Test".to_string());
        let result = dialog.on_event(UiEvent::Key(key(KeyCode::Tab)));

        assert!(result.handled);
        assert!(dialog.is_cancel_focused());
        assert!(!dialog.is_confirm_focused());
    }

    #[test]
    fn backtab_from_confirm_goes_to_cancel() {
        let mut dialog = Dialog::new("Test".to_string());
        let result = dialog.on_event(UiEvent::Key(key(KeyCode::BackTab)));

        assert!(result.handled);
        assert!(dialog.is_cancel_focused());
    }

    #[test]
    fn tab_at_cancel_stays_in_scope() {
        let mut dialog = Dialog::new("Test".to_string());
        dialog.focus_cancel();

        let result = dialog.on_event(UiEvent::Key(key(KeyCode::Tab)));

        // Since it's a scope, Tab from last item should stay consumed but not exit
        assert!(result.handled);
    }

    #[test]
    fn enter_confirms() {
        let mut dialog = Dialog::new("Test".to_string());
        let result = dialog.on_event(UiEvent::Key(key(KeyCode::Enter)));

        assert_eq!(result.messages, vec![DialogMessage::Confirm]);
    }

    #[test]
    fn escape_cancels() {
        let mut dialog = Dialog::new("Test".to_string());
        let result = dialog.on_event(UiEvent::Key(key(KeyCode::Esc)));

        assert_eq!(result.messages, vec![DialogMessage::Cancel]);
    }

    #[test]
    fn custom_labels() {
        let dialog = Dialog::new("Save?".to_string())
            .with_confirm_label("Save")
            .with_cancel_label("Discard");

        let lines = dialog.render(&test_context());
        let buttons_line = lines
            .iter()
            .find(|l| l.plain_text().contains("Discard"))
            .unwrap();
        let text = buttons_line.plain_text();
        assert!(text.contains("Save"));
        assert!(text.contains("Discard"));
    }

    #[test]
    fn render_shows_message() {
        let dialog = Dialog::new("Delete this item?".to_string());
        let lines = dialog.render(&test_context());

        let has_message = lines
            .iter()
            .any(|l| l.plain_text().contains("Delete this item?"));
        assert!(has_message, "Dialog should render the message");
    }

    #[test]
    fn render_shows_focused_indicator() {
        let dialog = Dialog::new("Test".to_string());
        let lines = dialog.render(&test_context());

        // Confirm should have the ▶ indicator
        let confirm_line = lines.iter().find(|l| l.plain_text().contains("▶")).unwrap();
        assert!(confirm_line.plain_text().contains(&dialog.confirm_label));
    }

    #[test]
    fn programmatic_focus() {
        let mut dialog = Dialog::new("Test".to_string());
        assert!(dialog.is_confirm_focused());

        dialog.focus_cancel();
        assert!(dialog.is_cancel_focused());
        assert!(!dialog.is_confirm_focused());

        dialog.focus_confirm();
        assert!(dialog.is_confirm_focused());
    }
}

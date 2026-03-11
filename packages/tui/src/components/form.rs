use crate::components::{Outcome, ViewContext, Widget, WidgetEvent};
use crate::focus::FocusRing;
use crate::line::Line;
use crate::style::Style;

use super::checkbox::Checkbox;
use super::multi_select::MultiSelect;
use super::number_field::NumberField;
use super::radio_select::RadioSelect;
use super::text_field::TextField;
use crossterm::event::KeyCode;

/// Messages emitted by [`Form`] input handling.
pub enum FormMessage {
    Close,
    Submit,
}

/// A multi-field form with Tab/`BackTab` focus navigation.
///
/// Renders a bordered form with labeled fields. Supports text, number, boolean,
/// single-select, and multi-select field types.
pub struct Form {
    pub message: String,
    pub fields: Vec<FormField>,
    focus: FocusRing,
}

/// A single field within a [`Form`].
pub struct FormField {
    pub name: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub kind: FormFieldKind,
}

/// The widget type backing a [`FormField`].
pub enum FormFieldKind {
    Text(TextField),
    Number(NumberField),
    Boolean(Checkbox),
    SingleSelect(RadioSelect),
    MultiSelect(MultiSelect),
}

impl Form {
    pub fn new(message: String, fields: Vec<FormField>) -> Self {
        let len = fields.len();
        Self {
            message,
            fields,
            focus: FocusRing::new(len),
        }
    }

    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for field in &self.fields {
            map.insert(field.name.clone(), field.kind.to_json());
        }
        serde_json::Value::Object(map)
    }

    fn render_title(&self, context: &ViewContext) -> Line {
        let title = format!("  {} ", self.message);
        let width = context.size.width as usize;
        let border_len = width.saturating_sub(title.len() + 4);
        Line::styled(
            format!("┌─{title}{}┐", "─".repeat(border_len)),
            context.theme.primary(),
        )
    }

    fn render_fields(&self, context: &ViewContext) -> Vec<Line> {
        let mut lines = Vec::new();
        for (i, field) in self.fields.iter().enumerate() {
            let is_selected = self.focus.is_focused(i);
            let prefix = if is_selected { "▶ " } else { "  " };
            let required_marker = if field.required { "*" } else { "" };
            let label_style = if is_selected {
                Style::fg(context.theme.primary()).bold()
            } else {
                Style::fg(context.theme.text_primary())
            };

            let mut label_line = Line::with_style(
                format!("│ {prefix}{}{required_marker}: ", field.label),
                label_style,
            );

            let field_lines = field.kind.render(&context.with_focused(is_selected));
            if let Some((first, rest)) = field_lines.split_first() {
                label_line.append_line(first);
                lines.push(label_line);

                if is_selected {
                    if let Some(desc) = &field.description {
                        lines.push(Line::styled(format!("│     {desc}"), context.theme.muted()));
                    }

                    for extra_line in rest {
                        let mut prefixed = Line::with_style("│       ", Style::default());
                        prefixed.append_line(extra_line);
                        lines.push(prefixed);
                    }
                }
            } else {
                lines.push(label_line);
            }
        }
        lines
    }

    fn render_footer(context: &ViewContext) -> Vec<Line> {
        let border_width = context.size.width.saturating_sub(2) as usize;
        vec![
            Line::styled(
                format!("│ {}", "[Enter] Submit  [Esc] Cancel"),
                context.theme.muted(),
            ),
            Line::styled(
                format!("└{}┘", "─".repeat(border_width)),
                context.theme.primary(),
            ),
        ]
    }
}

impl FormFieldKind {
    #[cfg(feature = "serde")]
    fn to_json(&self) -> serde_json::Value {
        match self {
            FormFieldKind::Text(w) => w.to_json(),
            FormFieldKind::Number(w) => w.to_json(),
            FormFieldKind::Boolean(w) => w.to_json(),
            FormFieldKind::SingleSelect(w) => w.to_json(),
            FormFieldKind::MultiSelect(w) => w.to_json(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        match self {
            FormFieldKind::Text(w) => w.render(context),
            FormFieldKind::Number(w) => w.render(context),
            FormFieldKind::Boolean(w) => w.render(context),
            FormFieldKind::SingleSelect(w) => w.render(context),
            FormFieldKind::MultiSelect(w) => w.render(context),
        }
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> Outcome<()> {
        match self {
            FormFieldKind::Text(w) => w.on_event(event),
            FormFieldKind::Number(w) => w.on_event(event),
            FormFieldKind::Boolean(w) => w.on_event(event),
            FormFieldKind::SingleSelect(w) => w.on_event(event),
            FormFieldKind::MultiSelect(w) => w.on_event(event),
        }
    }
}

impl Widget for Form {
    type Message = FormMessage;

    fn on_event(&mut self, event: &WidgetEvent) -> Outcome<Self::Message> {
        let WidgetEvent::Key(key) = event else {
            return Outcome::ignored();
        };
        match key.code {
            KeyCode::Esc => return Outcome::message(FormMessage::Close),
            KeyCode::Enter => return Outcome::message(FormMessage::Submit),
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus.handle_key(*key);
                return Outcome::consumed();
            }
            _ => {}
        }

        if let Some(field) = self.fields.get_mut(self.focus.focused()) {
            let result = field.kind.handle_event(event);
            if result.is_handled() {
                return result.discard_messages();
            }
        }
        Outcome::consumed()
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        let mut lines = vec![self.render_title(context)];
        lines.extend(self.render_fields(context));
        lines.extend(Self::render_footer(context));
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_does_not_panic_when_title_wider_than_terminal() {
        let form = Form::new(
            "This is a very long message that exceeds the terminal width".to_string(),
            vec![FormField {
                name: "name".to_string(),
                label: "Name".to_string(),
                description: None,
                required: false,
                kind: FormFieldKind::Text(TextField::new(String::new())),
            }],
        );
        let context = ViewContext::new((10, 10));

        // Should not panic with "attempt to subtract with overflow"
        let lines = form.render(&context);
        assert!(!lines.is_empty());
    }
}

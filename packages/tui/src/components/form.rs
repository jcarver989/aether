use crate::components::{Component, Event, ViewContext};
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

            let field_lines = field.kind.render_field(context, is_selected);
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

macro_rules! dispatch_field {
    ($self:expr, $w:ident => $body:expr) => {
        match $self {
            FormFieldKind::Text($w) => $body,
            FormFieldKind::Number($w) => $body,
            FormFieldKind::Boolean($w) => $body,
            FormFieldKind::SingleSelect($w) => $body,
            FormFieldKind::MultiSelect($w) => $body,
        }
    };
}

impl FormFieldKind {
    fn to_json(&self) -> serde_json::Value {
        dispatch_field!(self, w => w.to_json())
    }

    fn render_field(&self, context: &ViewContext, focused: bool) -> Vec<Line> {
        dispatch_field!(self, w => w.render_field(context, focused))
    }

    fn handle_event(&mut self, event: &Event) -> Option<Vec<()>> {
        dispatch_field!(self, w => w.on_event(event))
    }
}

impl Component for Form {
    type Message = FormMessage;

    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(key) = event else {
            return None;
        };
        match key.code {
            KeyCode::Esc => return Some(vec![FormMessage::Close]),
            KeyCode::Enter => return Some(vec![FormMessage::Submit]),
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus.handle_key(*key);
                return Some(vec![]);
            }
            _ => {}
        }

        if let Some(field) = self.fields.get_mut(self.focus.focused()) {
            let result = field.kind.handle_event(event);
            if result.is_some() {
                return Some(vec![]);
            }
        }
        Some(vec![])
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

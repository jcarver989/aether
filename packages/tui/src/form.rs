use crate::screen::{Line, Style};
use crate::{
    Checkbox, Component, HandlesInput, InputOutcome, MultiSelect, NumberField, RadioSelect,
    RenderContext, TextField,
};
use crossterm::event::{KeyCode, KeyEvent};

pub enum FormAction {
    Close,
    Submit,
}

pub struct Form {
    pub message: String,
    pub fields: Vec<FormField>,
    pub selected_field: usize,
}

pub struct FormField {
    pub name: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub kind: FormFieldKind,
}

pub enum FormFieldKind {
    Text(TextField),
    Number(NumberField),
    Boolean(Checkbox),
    SingleSelect(RadioSelect),
    MultiSelect(MultiSelect),
}

impl Form {
    pub fn new(message: String, fields: Vec<FormField>) -> Self {
        Self {
            message,
            fields,
            selected_field: 0,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for field in &self.fields {
            map.insert(field.name.clone(), field.kind.to_json());
        }
        serde_json::Value::Object(map)
    }
}

impl FormFieldKind {
    fn to_json(&self) -> serde_json::Value {
        match self {
            FormFieldKind::Text(w) => w.to_json(),
            FormFieldKind::Number(w) => w.to_json(),
            FormFieldKind::Boolean(w) => w.to_json(),
            FormFieldKind::SingleSelect(w) => w.to_json(),
            FormFieldKind::MultiSelect(w) => w.to_json(),
        }
    }

    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        match self {
            FormFieldKind::Text(w) => w.render(context),
            FormFieldKind::Number(w) => w.render(context),
            FormFieldKind::Boolean(w) => w.render(context),
            FormFieldKind::SingleSelect(w) => w.render(context),
            FormFieldKind::MultiSelect(w) => w.render(context),
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<()> {
        match self {
            FormFieldKind::Text(w) => w.handle_key(key_event),
            FormFieldKind::Number(w) => w.handle_key(key_event),
            FormFieldKind::Boolean(w) => w.handle_key(key_event),
            FormFieldKind::SingleSelect(w) => w.handle_key(key_event),
            FormFieldKind::MultiSelect(w) => w.handle_key(key_event),
        }
    }
}

impl Component for Form {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut lines = vec![self.render_title(context)];
        lines.extend(self.render_fields(context));
        lines.extend(Self::render_footer(context));
        lines
    }
}

impl Form {
    fn render_title(&self, context: &RenderContext) -> Line {
        let title = format!("  {} ", self.message);
        let width = context.size.width as usize;
        let border_len = width.saturating_sub(title.len() + 4);
        Line::styled(
            format!("┌─{title}{}┐", "─".repeat(border_len)),
            context.theme.primary(),
        )
    }

    fn render_fields(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        for (i, field) in self.fields.iter_mut().enumerate() {
            let is_selected = i == self.selected_field;
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

    fn render_footer(context: &RenderContext) -> Vec<Line> {
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

impl HandlesInput for Form {
    type Action = FormAction;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        match key_event.code {
            KeyCode::Esc => InputOutcome::action_and_render(FormAction::Close),
            KeyCode::Enter => InputOutcome::action_and_render(FormAction::Submit),
            KeyCode::Tab => {
                if !self.fields.is_empty() {
                    self.selected_field = (self.selected_field + 1) % self.fields.len();
                }
                InputOutcome::consumed_and_render()
            }
            KeyCode::BackTab => {
                if !self.fields.is_empty() {
                    self.selected_field =
                        (self.selected_field + self.fields.len() - 1) % self.fields.len();
                }
                InputOutcome::consumed_and_render()
            }
            _ => {
                if let Some(field) = self.fields.get_mut(self.selected_field) {
                    let outcome = field.kind.handle_key(key_event);
                    if outcome.consumed {
                        return InputOutcome {
                            consumed: true,
                            needs_render: outcome.needs_render,
                            action: None,
                        };
                    }
                }
                InputOutcome::consumed()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_does_not_panic_when_title_wider_than_terminal() {
        let mut form = Form::new(
            "This is a very long message that exceeds the terminal width".to_string(),
            vec![FormField {
                name: "name".to_string(),
                label: "Name".to_string(),
                description: None,
                required: false,
                kind: FormFieldKind::Text(TextField::new(String::new())),
            }],
        );
        let context = RenderContext::new((10, 10));

        // Should not panic with "attempt to subtract with overflow"
        let lines = form.render(&context);
        assert!(!lines.is_empty());
    }
}

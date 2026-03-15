use crate::tui::{Checkbox, MultiSelect, NumberField, RadioSelect, SelectOption, TextField};
use crate::tui::{Component, Event, Form, FormField, FormFieldKind, FormMessage, Frame, ViewContext};
use acp_utils::notifications::{ElicitationAction, ElicitationParams, ElicitationResponse};
use acp_utils::{
    ConstTitle, ElicitationSchema, EnumSchema, MultiSelectEnumSchema, PrimitiveSchema,
    SingleSelectEnumSchema,
};
use tokio::sync::oneshot;

pub enum ElicitationMessage {
    Responded,
}

pub struct ElicitationForm {
    pub form: Form,
    pub(crate) response_tx: Option<oneshot::Sender<ElicitationResponse>>,
}

impl Component for ElicitationForm {
    type Message = ElicitationMessage;

    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let outcome = self.form.on_event(event)?;
        if let Some(msg) = outcome.into_iter().next() {
            match msg {
                FormMessage::Close => {
                    let _ = self.response_tx.take().map(|tx| tx.send(Self::decline()));
                    return Some(vec![ElicitationMessage::Responded]);
                }
                FormMessage::Submit => {
                    let response = self.confirm();
                    let _ = self.response_tx.take().map(|tx| tx.send(response));
                    return Some(vec![ElicitationMessage::Responded]);
                }
            }
        }
        Some(vec![])
    }

    fn render(&self, ctx: &ViewContext) -> Frame {
        self.form.render(ctx)
    }
}

impl ElicitationForm {
    pub fn from_params(
        params: ElicitationParams,
        response_tx: oneshot::Sender<ElicitationResponse>,
    ) -> Self {
        let fields = parse_schema(&params.schema);
        Self {
            form: Form::new(params.message, fields),
            response_tx: Some(response_tx),
        }
    }

    pub fn confirm(&self) -> ElicitationResponse {
        ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(self.form.to_json()),
        }
    }

    pub fn decline() -> ElicitationResponse {
        ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
        }
    }
}

fn parse_schema(schema: &ElicitationSchema) -> Vec<FormField> {
    let required = schema.required.as_deref().unwrap_or(&[]);
    schema
        .properties
        .iter()
        .map(|(name, prop)| {
            let (title, description) = extract_metadata(prop);
            FormField {
                name: name.clone(),
                label: title.unwrap_or_else(|| name.clone()),
                description,
                required: required.iter().any(|r| r == name),
                kind: parse_field_kind(prop),
            }
        })
        .collect()
}

fn parse_field_kind(prop: &PrimitiveSchema) -> FormFieldKind {
    match prop {
        PrimitiveSchema::Boolean(b) => {
            FormFieldKind::Boolean(Checkbox::new(b.default.unwrap_or(false)))
        }
        PrimitiveSchema::Integer(_) => FormFieldKind::Number(NumberField::new(String::new(), true)),
        PrimitiveSchema::Number(_) => FormFieldKind::Number(NumberField::new(String::new(), false)),
        PrimitiveSchema::String(_) => FormFieldKind::Text(TextField::new(String::new())),
        PrimitiveSchema::Enum(e) => parse_enum_field(e),
    }
}

fn parse_enum_field(e: &EnumSchema) -> FormFieldKind {
    match e {
        EnumSchema::Single(s) => match s {
            SingleSelectEnumSchema::Untitled(u) => {
                let options = options_from_strings(&u.enum_);
                let default_idx = u
                    .default
                    .as_ref()
                    .and_then(|d| options.iter().position(|o| o.value == *d))
                    .unwrap_or(0);
                FormFieldKind::SingleSelect(RadioSelect::new(options, default_idx))
            }
            SingleSelectEnumSchema::Titled(t) => {
                let options = options_from_const_titles(&t.one_of);
                let default_idx = t
                    .default
                    .as_ref()
                    .and_then(|d| options.iter().position(|o| o.value == *d))
                    .unwrap_or(0);
                FormFieldKind::SingleSelect(RadioSelect::new(options, default_idx))
            }
        },
        EnumSchema::Multi(m) => match m {
            MultiSelectEnumSchema::Untitled(u) => {
                let options = options_from_strings(&u.items.enum_);
                let defaults = u.default.as_deref().unwrap_or(&[]);
                let selected: Vec<bool> = options
                    .iter()
                    .map(|o| defaults.contains(&o.value))
                    .collect();
                FormFieldKind::MultiSelect(MultiSelect::new(options, selected))
            }
            MultiSelectEnumSchema::Titled(t) => {
                let options = options_from_const_titles(&t.items.any_of);
                let defaults = t.default.as_deref().unwrap_or(&[]);
                let selected: Vec<bool> = options
                    .iter()
                    .map(|o| defaults.contains(&o.value))
                    .collect();
                FormFieldKind::MultiSelect(MultiSelect::new(options, selected))
            }
        },
        EnumSchema::Legacy(l) => {
            let options = options_from_strings(&l.enum_);
            FormFieldKind::SingleSelect(RadioSelect::new(options, 0))
        }
    }
}

fn extract_metadata(prop: &PrimitiveSchema) -> (Option<String>, Option<String>) {
    match prop {
        PrimitiveSchema::String(s) => (
            s.title.as_ref().map(ToString::to_string),
            s.description.as_ref().map(ToString::to_string),
        ),
        PrimitiveSchema::Number(n) => (
            n.title.as_ref().map(ToString::to_string),
            n.description.as_ref().map(ToString::to_string),
        ),
        PrimitiveSchema::Integer(i) => (
            i.title.as_ref().map(ToString::to_string),
            i.description.as_ref().map(ToString::to_string),
        ),
        PrimitiveSchema::Boolean(b) => (
            b.title.as_ref().map(ToString::to_string),
            b.description.as_ref().map(ToString::to_string),
        ),
        PrimitiveSchema::Enum(e) => extract_enum_metadata(e),
    }
}

fn extract_enum_metadata(e: &EnumSchema) -> (Option<String>, Option<String>) {
    match e {
        EnumSchema::Single(s) => match s {
            SingleSelectEnumSchema::Untitled(u) => (
                u.title.as_ref().map(ToString::to_string),
                u.description.as_ref().map(ToString::to_string),
            ),
            SingleSelectEnumSchema::Titled(t) => (
                t.title.as_ref().map(ToString::to_string),
                t.description.as_ref().map(ToString::to_string),
            ),
        },
        EnumSchema::Multi(m) => match m {
            MultiSelectEnumSchema::Untitled(u) => (
                u.title.as_ref().map(ToString::to_string),
                u.description.as_ref().map(ToString::to_string),
            ),
            MultiSelectEnumSchema::Titled(t) => (
                t.title.as_ref().map(ToString::to_string),
                t.description.as_ref().map(ToString::to_string),
            ),
        },
        EnumSchema::Legacy(l) => (
            l.title.as_ref().map(ToString::to_string),
            l.description.as_ref().map(ToString::to_string),
        ),
    }
}

fn options_from_strings(values: &[String]) -> Vec<SelectOption> {
    values
        .iter()
        .map(|s| SelectOption {
            value: s.clone(),
            title: s.clone(),
        })
        .collect()
}

fn options_from_const_titles(items: &[ConstTitle]) -> Vec<SelectOption> {
    items
        .iter()
        .map(|ct| SelectOption {
            value: ct.const_.clone(),
            title: ct.title.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_utils::EnumSchema;
    use std::collections::BTreeMap;

    fn test_schema() -> ElicitationSchema {
        serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "title": "Your Name",
                    "description": "Enter your full name"
                },
                "age": {
                    "type": "integer",
                    "title": "Age",
                    "minimum": 0,
                    "maximum": 150
                },
                "rating": {
                    "type": "number",
                    "title": "Rating"
                },
                "approved": {
                    "type": "boolean",
                    "title": "Approved",
                    "default": true
                },
                "color": {
                    "type": "string",
                    "title": "Favorite Color",
                    "enum": ["red", "green", "blue"]
                },
                "tags": {
                    "type": "array",
                    "title": "Tags",
                    "items": {
                        "type": "string",
                        "enum": ["fast", "reliable", "cheap"]
                    }
                }
            },
            "required": ["name", "color"]
        }))
        .unwrap()
    }

    #[test]
    fn parse_schema_extracts_all_field_types() {
        let schema = test_schema();
        let fields = parse_schema(&schema);
        assert_eq!(fields.len(), 6);

        let name_field = fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name_field.label, "Your Name");
        assert!(name_field.required);
        assert!(matches!(name_field.kind, FormFieldKind::Text(_)));

        let age_field = fields.iter().find(|f| f.name == "age").unwrap();
        match &age_field.kind {
            FormFieldKind::Number(nf) => assert!(nf.integer_only),
            _ => panic!("Expected Number (integer)"),
        }

        let bool_field = fields.iter().find(|f| f.name == "approved").unwrap();
        match &bool_field.kind {
            FormFieldKind::Boolean(cb) => assert!(cb.checked),
            _ => panic!("Expected Boolean"),
        }

        let color_field = fields.iter().find(|f| f.name == "color").unwrap();
        assert!(color_field.required);
        match &color_field.kind {
            FormFieldKind::SingleSelect(rs) => {
                assert_eq!(rs.options.len(), 3);
                assert_eq!(rs.options[0].value, "red");
            }
            _ => panic!("Expected SingleSelect"),
        }

        let tags_field = fields.iter().find(|f| f.name == "tags").unwrap();
        match &tags_field.kind {
            FormFieldKind::MultiSelect(ms) => {
                assert_eq!(ms.options.len(), 3);
                assert!(ms.selected.iter().all(|&s| !s));
            }
            _ => panic!("Expected MultiSelect"),
        }
    }

    #[test]
    fn confirm_produces_correct_json() {
        let (tx, _rx) = oneshot::channel();
        let params = ElicitationParams {
            message: "Test".to_string(),
            schema: ElicitationSchema::builder()
                .optional_string("name")
                .optional_bool("approved", true)
                .optional_enum_schema(
                    "color",
                    EnumSchema::builder(vec!["red".into(), "green".into()])
                        .untitled()
                        .with_default("green")
                        .unwrap()
                        .build(),
                )
                .build()
                .unwrap(),
        };

        let form = ElicitationForm::from_params(params, tx);
        let response = form.confirm();

        assert_eq!(response.action, ElicitationAction::Accept);
        let content = response.content.unwrap();
        assert_eq!(content["name"], "");
        assert_eq!(content["approved"], true);
        assert_eq!(content["color"], "green");
    }

    #[test]
    fn esc_returns_decline() {
        let response = ElicitationForm::decline();
        assert_eq!(response.action, ElicitationAction::Decline);
        assert!(response.content.is_none());
    }

    #[test]
    fn one_of_string_produces_single_select() {
        let schema: ElicitationSchema = serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "size": {
                    "type": "string",
                    "oneOf": [
                        { "const": "s", "title": "Small" },
                        { "const": "m", "title": "Medium" },
                        { "const": "l", "title": "Large" }
                    ]
                }
            }
        }))
        .unwrap();
        let fields = parse_schema(&schema);
        assert_eq!(fields.len(), 1);
        match &fields[0].kind {
            FormFieldKind::SingleSelect(rs) => {
                assert_eq!(rs.options.len(), 3);
                assert_eq!(rs.options[0].title, "Small");
                assert_eq!(rs.options[0].value, "s");
            }
            _ => panic!("Expected SingleSelect"),
        }
    }

    #[test]
    fn empty_schema_produces_no_fields() {
        let schema = ElicitationSchema::new(BTreeMap::new());
        let fields = parse_schema(&schema);
        assert!(fields.is_empty());
    }
}

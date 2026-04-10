use acp_utils::notifications::{
    CreateElicitationRequestParams, ElicitationAction, ElicitationParams, ElicitationResponse,
};
use acp_utils::{
    ConstTitle, ElicitationSchema, EnumSchema, MultiSelectEnumSchema, PrimitiveSchema, SingleSelectEnumSchema,
};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::oneshot;
use tui::{
    Checkbox, Component, Event, Form, FormField, FormFieldKind, FormMessage, Frame, MultiSelect, NumberField,
    RadioSelect, SelectOption, TextField, ViewContext,
};

pub enum ElicitationMessage {
    Responded,
    /// Emitted when a URL modal successfully opens the browser.
    UrlOpened {
        elicitation_id: String,
        server_name: String,
    },
}

pub enum ElicitationUi {
    Form(Form),
    Url(UrlPrompt),
}

pub struct UrlPrompt {
    pub server_name: String,
    pub elicitation_id: String,
    pub message: String,
    pub url: String,
    pub host: Option<String>,
    pub warnings: Vec<String>,
    pub launch_error: Option<String>,
}

type BrowserOpener = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

pub struct ElicitationForm {
    pub ui: ElicitationUi,
    browser_opener: BrowserOpener,
    pub(crate) response_tx: Option<oneshot::Sender<ElicitationResponse>>,
}

impl UrlPrompt {
    pub fn new(server_name: String, elicitation_id: String, message: String, url: String) -> Self {
        let parsed_url = url::Url::parse(&url);
        let host = parsed_url.as_ref().ok().and_then(|parsed| parsed.host_str().map(std::string::ToString::to_string));

        let mut warnings = Vec::new();
        match parsed_url {
            Ok(parsed_url) => {
                if let Some(ref h) = host
                    && h.contains("xn--")
                {
                    warnings.push(
                        "Warning: URL contains punycode (internationalized domain). Verify the domain before proceeding."
                            .to_string(),
                    );
                }
                if parsed_url.scheme() != "https" && !is_local_http_url(&parsed_url) {
                    warnings.push("Warning: URL does not use HTTPS.".to_string());
                }
            }
            Err(_) => {
                warnings.push("Warning: URL could not be parsed. Verify it carefully before proceeding.".to_string());
            }
        }

        Self { server_name, elicitation_id, message, url, host, warnings, launch_error: None }
    }
}

impl Component for ElicitationForm {
    type Message = ElicitationMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        match &mut self.ui {
            ElicitationUi::Form(form) => {
                let outcome = form.on_event(event).await?;
                if let Some(msg) = outcome.into_iter().next() {
                    match msg {
                        FormMessage::Close => {
                            let _ = self.response_tx.take().map(|tx| tx.send(Self::cancel()));
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
            ElicitationUi::Url(prompt) => {
                let Event::Key(key) = event else {
                    return Some(vec![]);
                };
                match key.code {
                    tui::KeyCode::Enter => match (self.browser_opener)(&prompt.url) {
                        Ok(()) => {
                            let server_name = prompt.server_name.clone();
                            let elicitation_id = prompt.elicitation_id.clone();
                            let _ = self.response_tx.take().map(|tx| {
                                tx.send(ElicitationResponse { action: ElicitationAction::Accept, content: None })
                            });
                            return Some(vec![
                                ElicitationMessage::Responded,
                                ElicitationMessage::UrlOpened { elicitation_id, server_name },
                            ]);
                        }
                        Err(e) => {
                            prompt.launch_error = Some(format!("Failed to open browser: {e}"));
                        }
                    },
                    tui::KeyCode::Char('d' | 'D') => {
                        let _ = self.response_tx.take().map(|tx| tx.send(Self::decline()));
                        return Some(vec![ElicitationMessage::Responded]);
                    }
                    tui::KeyCode::Esc => {
                        let _ = self.response_tx.take().map(|tx| tx.send(Self::cancel()));
                        return Some(vec![ElicitationMessage::Responded]);
                    }
                    _ => {}
                }
                Some(vec![])
            }
        }
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        match &mut self.ui {
            ElicitationUi::Form(form) => form.render(ctx),
            ElicitationUi::Url(prompt) => render_url_prompt(prompt, ctx),
        }
    }
}

impl ElicitationForm {
    pub fn from_params(params: ElicitationParams, response_tx: oneshot::Sender<ElicitationResponse>) -> Self {
        Self::with_browser_opener(params, response_tx, default_browser_opener)
    }

    pub fn with_browser_opener<F>(
        params: ElicitationParams,
        response_tx: oneshot::Sender<ElicitationResponse>,
        browser_opener: F,
    ) -> Self
    where
        F: Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    {
        let ui = match params.request {
            CreateElicitationRequestParams::FormElicitationParams { message, requested_schema, .. } => {
                let fields = parse_schema(&requested_schema);
                ElicitationUi::Form(Form::new(message, fields))
            }
            CreateElicitationRequestParams::UrlElicitationParams { message, url, elicitation_id, .. } => {
                ElicitationUi::Url(UrlPrompt::new(params.server_name, elicitation_id, message, url))
            }
        };
        Self { ui, browser_opener: Arc::new(browser_opener), response_tx: Some(response_tx) }
    }

    pub fn confirm(&self) -> ElicitationResponse {
        match &self.ui {
            ElicitationUi::Form(form) => {
                ElicitationResponse { action: ElicitationAction::Accept, content: Some(form.to_json()) }
            }
            ElicitationUi::Url(_) => ElicitationResponse { action: ElicitationAction::Accept, content: None },
        }
    }

    pub fn decline() -> ElicitationResponse {
        ElicitationResponse { action: ElicitationAction::Decline, content: None }
    }

    pub fn cancel() -> ElicitationResponse {
        ElicitationResponse { action: ElicitationAction::Cancel, content: None }
    }
}

fn render_url_prompt(prompt: &UrlPrompt, ctx: &ViewContext) -> Frame {
    use tui::{Line, Style};

    let mut lines = Vec::new();
    let primary = ctx.theme.primary();
    let text_primary = ctx.theme.text_primary();
    let text_secondary = ctx.theme.text_secondary();
    let warning_color = ctx.theme.warning();
    let muted = ctx.theme.muted();

    lines.push(Line::with_style(
        format!("{} requests you to open a URL", prompt.server_name),
        Style::fg(primary).bold(),
    ));
    lines.push(Line::default());
    lines.push(Line::with_style(&prompt.message, Style::fg(text_primary)));
    lines.push(Line::default());
    lines.push(Line::with_style("URL:", Style::fg(text_secondary)));
    lines.push(Line::with_style(&prompt.url, Style::fg(primary)));

    if let Some(ref host) = prompt.host {
        lines.push(Line::with_style(format!("Host: {host}"), Style::fg(text_secondary)));
    }

    if !prompt.warnings.is_empty() {
        lines.push(Line::default());
        for warning in &prompt.warnings {
            lines.push(Line::styled(warning, warning_color));
        }
    }

    if let Some(ref error) = prompt.launch_error {
        lines.push(Line::default());
        lines.push(Line::styled(error, ctx.theme.error()));
    }

    lines.push(Line::default());
    lines.push(Line::styled("Enter to open URL · D to decline · Esc to cancel", muted));

    Frame::new(lines)
}

fn is_local_http_url(url: &url::Url) -> bool {
    if url.scheme() != "http" {
        return false;
    }

    matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
}

fn default_browser_opener(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open").arg(url).status().map_err(|e| e.to_string())?;
        return status.success().then_some(()).ok_or_else(|| format!("open exited with status {status}"));
    }

    #[cfg(target_os = "linux")]
    {
        let status = Command::new("xdg-open").arg(url).status().map_err(|e| e.to_string())?;
        return status.success().then_some(()).ok_or_else(|| format!("xdg-open exited with status {status}"));
    }

    #[cfg(target_os = "windows")]
    {
        let status = Command::new("cmd").args(["/C", "start", url]).status().map_err(|e| e.to_string())?;
        return status.success().then_some(()).ok_or_else(|| format!("start exited with status {status}"));
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform for opening URLs".to_string())
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
        PrimitiveSchema::Boolean(b) => FormFieldKind::Boolean(Checkbox::new(b.default.unwrap_or(false))),
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
                let default_idx =
                    u.default.as_ref().and_then(|d| options.iter().position(|o| o.value == *d)).unwrap_or(0);
                FormFieldKind::SingleSelect(RadioSelect::new(options, default_idx))
            }
            SingleSelectEnumSchema::Titled(t) => {
                let options = options_from_const_titles(&t.one_of);
                let default_idx =
                    t.default.as_ref().and_then(|d| options.iter().position(|o| o.value == *d)).unwrap_or(0);
                FormFieldKind::SingleSelect(RadioSelect::new(options, default_idx))
            }
        },
        EnumSchema::Multi(m) => match m {
            MultiSelectEnumSchema::Untitled(u) => {
                let options = options_from_strings(&u.items.enum_);
                let defaults = u.default.as_deref().unwrap_or(&[]);
                let selected: Vec<bool> = options.iter().map(|o| defaults.contains(&o.value)).collect();
                FormFieldKind::MultiSelect(MultiSelect::new(options, selected))
            }
            MultiSelectEnumSchema::Titled(t) => {
                let options = options_from_const_titles(&t.items.any_of);
                let defaults = t.default.as_deref().unwrap_or(&[]);
                let selected: Vec<bool> = options.iter().map(|o| defaults.contains(&o.value)).collect();
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
        PrimitiveSchema::String(s) => {
            (s.title.as_ref().map(ToString::to_string), s.description.as_ref().map(ToString::to_string))
        }
        PrimitiveSchema::Number(n) => {
            (n.title.as_ref().map(ToString::to_string), n.description.as_ref().map(ToString::to_string))
        }
        PrimitiveSchema::Integer(i) => {
            (i.title.as_ref().map(ToString::to_string), i.description.as_ref().map(ToString::to_string))
        }
        PrimitiveSchema::Boolean(b) => {
            (b.title.as_ref().map(ToString::to_string), b.description.as_ref().map(ToString::to_string))
        }
        PrimitiveSchema::Enum(e) => extract_enum_metadata(e),
    }
}

fn extract_enum_metadata(e: &EnumSchema) -> (Option<String>, Option<String>) {
    match e {
        EnumSchema::Single(s) => match s {
            SingleSelectEnumSchema::Untitled(u) => {
                (u.title.as_ref().map(ToString::to_string), u.description.as_ref().map(ToString::to_string))
            }
            SingleSelectEnumSchema::Titled(t) => {
                (t.title.as_ref().map(ToString::to_string), t.description.as_ref().map(ToString::to_string))
            }
        },
        EnumSchema::Multi(m) => match m {
            MultiSelectEnumSchema::Untitled(u) => {
                (u.title.as_ref().map(ToString::to_string), u.description.as_ref().map(ToString::to_string))
            }
            MultiSelectEnumSchema::Titled(t) => {
                (t.title.as_ref().map(ToString::to_string), t.description.as_ref().map(ToString::to_string))
            }
        },
        EnumSchema::Legacy(l) => {
            (l.title.as_ref().map(ToString::to_string), l.description.as_ref().map(ToString::to_string))
        }
    }
}

fn options_from_strings(values: &[String]) -> Vec<SelectOption> {
    values.iter().map(|s| SelectOption { value: s.clone(), title: s.clone(), description: None }).collect()
}

fn options_from_const_titles(items: &[ConstTitle]) -> Vec<SelectOption> {
    items
        .iter()
        .map(|ct| SelectOption { value: ct.const_.clone(), title: ct.title.clone(), description: None })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_utils::EnumSchema;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

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
            server_name: "test-server".to_string(),
            request: CreateElicitationRequestParams::FormElicitationParams {
                meta: None,
                message: "Test".to_string(),
                requested_schema: ElicitationSchema::builder()
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
            },
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
    fn esc_returns_cancel() {
        let response = ElicitationForm::cancel();
        assert_eq!(response.action, ElicitationAction::Cancel);
        assert!(response.content.is_none());
    }

    #[test]
    fn decline_returns_decline() {
        let response = ElicitationForm::decline();
        assert_eq!(response.action, ElicitationAction::Decline);
        assert!(response.content.is_none());
    }

    #[test]
    fn url_prompt_parses_host() {
        let prompt = UrlPrompt::new(
            "github".to_string(),
            "el-1".to_string(),
            "Authorize".to_string(),
            "https://github.com/login/oauth".to_string(),
        );
        assert_eq!(prompt.host.as_deref(), Some("github.com"));
        assert!(prompt.warnings.is_empty());
        assert!(prompt.launch_error.is_none());
    }

    #[test]
    fn url_prompt_warns_on_non_https() {
        let prompt = UrlPrompt::new(
            "test".to_string(),
            "el-1".to_string(),
            "Open this".to_string(),
            "http://example.com/form".to_string(),
        );
        assert_eq!(prompt.warnings.len(), 1);
        assert!(prompt.warnings[0].contains("HTTPS"));
    }

    #[test]
    fn url_prompt_does_not_warn_on_localhost() {
        let prompt = UrlPrompt::new(
            "test".to_string(),
            "el-1".to_string(),
            "Local".to_string(),
            "http://localhost:3000/auth".to_string(),
        );
        assert!(prompt.warnings.is_empty());
    }

    #[test]
    fn url_prompt_warns_on_invalid_url() {
        let prompt = UrlPrompt::new(
            "test".to_string(),
            "el-invalid".to_string(),
            "Check this".to_string(),
            "not a valid url".to_string(),
        );
        assert!(prompt.host.is_none());
        assert!(
            prompt.warnings.iter().any(|warning| warning.contains("could not be parsed")),
            "invalid URLs should show an explicit warning"
        );
    }

    #[test]
    fn url_prompt_warns_on_punycode() {
        let prompt = UrlPrompt::new(
            "test".to_string(),
            "el-1".to_string(),
            "Phishing".to_string(),
            "https://xn--e1afmkfd.xn--p1ai/".to_string(),
        );
        assert_eq!(prompt.warnings.len(), 1);
        assert!(prompt.warnings[0].contains("punycode"));
    }

    #[test]
    fn url_prompt_warns_on_punycode_and_non_https() {
        let prompt = UrlPrompt::new(
            "test".to_string(),
            "el-1".to_string(),
            "Both".to_string(),
            "http://xn--e1afmkfd.xn--p1ai/".to_string(),
        );
        assert_eq!(prompt.warnings.len(), 2, "both warnings should be present");
        assert!(prompt.warnings.iter().any(|w| w.contains("punycode")));
        assert!(prompt.warnings.iter().any(|w| w.contains("HTTPS")));
    }

    fn url_params(server: &str, id: &str, url: &str) -> ElicitationParams {
        ElicitationParams {
            server_name: server.to_string(),
            request: CreateElicitationRequestParams::UrlElicitationParams {
                meta: None,
                message: "Auth".to_string(),
                url: url.to_string(),
                elicitation_id: id.to_string(),
            },
        }
    }

    #[tokio::test]
    async fn url_modal_enter_returns_accept_with_carried_id() {
        let opened_urls = Arc::new(Mutex::new(Vec::new()));
        let opened_urls_for_opener = Arc::clone(&opened_urls);
        let (tx, rx) = oneshot::channel();
        let params = url_params("github", "el-123", "https://github.com/login/oauth");
        let mut form = ElicitationForm::with_browser_opener(params, tx, move |url| {
            opened_urls_for_opener.lock().unwrap().push(url.to_string());
            Ok(())
        });
        let outcome =
            form.on_event(&Event::Key(tui::KeyEvent::new(tui::KeyCode::Enter, tui::KeyModifiers::NONE))).await;
        let messages = outcome.unwrap();

        assert_eq!(opened_urls.lock().unwrap().as_slice(), ["https://github.com/login/oauth"]);
        assert!(messages.iter().any(|m| matches!(m, ElicitationMessage::Responded)));
        let opened = messages.iter().find_map(|m| match m {
            ElicitationMessage::UrlOpened { elicitation_id, server_name } => {
                Some((elicitation_id.clone(), server_name.clone()))
            }
            ElicitationMessage::Responded => None,
        });
        let (id, server) = opened.expect("UrlOpened message should be emitted");
        assert_eq!(id, "el-123", "elicitation_id must come from request, not URL re-parsing");
        assert_eq!(server, "github");

        let response = rx.await.unwrap();
        assert_eq!(response.action, ElicitationAction::Accept);
        assert!(response.content.is_none());
    }

    #[tokio::test]
    async fn url_modal_launch_failure_keeps_modal_open_and_shows_error() {
        let (tx, mut rx) = oneshot::channel();
        let params = url_params("github", "el-fail", "https://github.com/login/oauth");
        let mut form = ElicitationForm::with_browser_opener(params, tx, |_| Err("boom".to_string()));

        let outcome =
            form.on_event(&Event::Key(tui::KeyEvent::new(tui::KeyCode::Enter, tui::KeyModifiers::NONE))).await;
        let messages = outcome.expect("URL opener failure should still produce an event result");
        assert!(messages.is_empty(), "modal should remain open on launch failure");
        assert!(rx.try_recv().is_err(), "response should not be sent when browser launch fails");

        let ElicitationUi::Url(prompt) = &form.ui else {
            panic!("expected URL prompt");
        };
        assert_eq!(prompt.launch_error.as_deref(), Some("Failed to open browser: boom"));
    }

    #[tokio::test]
    async fn url_modal_d_returns_decline() {
        let (tx, rx) = oneshot::channel();
        let params = url_params("github", "el-456", "https://github.com/login/oauth");
        let mut form = ElicitationForm::from_params(params, tx);
        let outcome =
            form.on_event(&Event::Key(tui::KeyEvent::new(tui::KeyCode::Char('d'), tui::KeyModifiers::NONE))).await;
        let messages = outcome.unwrap();

        assert!(messages.iter().any(|m| matches!(m, ElicitationMessage::Responded)));

        let response = rx.await.unwrap();
        assert_eq!(response.action, ElicitationAction::Decline);
    }

    #[tokio::test]
    async fn url_modal_esc_returns_cancel() {
        let (tx, rx) = oneshot::channel();
        let params = url_params("github", "el-789", "https://github.com/login/oauth");
        let mut form = ElicitationForm::from_params(params, tx);
        let outcome = form.on_event(&Event::Key(tui::KeyEvent::new(tui::KeyCode::Esc, tui::KeyModifiers::NONE))).await;
        let messages = outcome.unwrap();

        assert!(messages.iter().any(|m| matches!(m, ElicitationMessage::Responded)));

        let response = rx.await.unwrap();
        assert_eq!(response.action, ElicitationAction::Cancel);
    }

    #[tokio::test]
    async fn form_modal_esc_returns_cancel() {
        let (tx, rx) = oneshot::channel();
        let params = ElicitationParams {
            server_name: "test".to_string(),
            request: CreateElicitationRequestParams::FormElicitationParams {
                meta: None,
                message: "Test".to_string(),
                requested_schema: ElicitationSchema::builder().build().unwrap(),
            },
        };
        let mut form = ElicitationForm::from_params(params, tx);
        let outcome = form.on_event(&Event::Key(tui::KeyEvent::new(tui::KeyCode::Esc, tui::KeyModifiers::NONE))).await;
        let messages = outcome.unwrap();

        assert!(messages.iter().any(|m| matches!(m, ElicitationMessage::Responded)));

        let response = rx.await.unwrap();
        assert_eq!(response.action, ElicitationAction::Cancel);
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

    #[test]
    fn url_modal_renders_server_name_and_url() {
        use tui::testing::render_component;

        let prompt = UrlPrompt::new(
            "github".to_string(),
            "el-1".to_string(),
            "Authorize GitHub".to_string(),
            "https://github.com/login/oauth".to_string(),
        );
        let ui = ElicitationUi::Url(prompt);
        let mut form = ElicitationForm { ui, browser_opener: Arc::new(default_browser_opener), response_tx: None };

        let lines = render_component(|ctx| form.render(ctx), 80, 20).get_lines();
        let text: String = lines.join("\n");
        assert!(text.contains("github"), "should show server name");
        assert!(text.contains("https://github.com/login/oauth"), "should show full URL");
        assert!(text.contains("github.com"), "should show host");
        assert!(text.contains("Enter to open URL"), "should show footer hint");
    }
}

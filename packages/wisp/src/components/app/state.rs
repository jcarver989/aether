use super::{AppEffect, theme_file_from_picker_value};
use crate::components::config_menu::ConfigMenu;
use crate::components::config_overlay::{ConfigOverlay, ConfigOverlayAction};
use crate::components::conversation_window::ConversationBuffer;
use crate::components::elicitation_form::ElicitationForm;
use crate::components::plan_tracker::PlanTracker;
use crate::components::progress_indicator::ProgressIndicator;
use crate::components::prompt_composer::{PromptComposer, PromptComposerAction};
use crate::components::server_status::server_status_summary;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::settings::{list_theme_files, load_or_create_settings};
use crate::tui::components::spinner::Spinner;
use crate::tui::{FormAction, InputOutcome, InteractiveComponent, Line};
use acp_utils::config_option_id::{ConfigOptionId, THEME_CONFIG_ID};
use acp_utils::notifications::{ElicitationParams, ElicitationResponse, McpServerStatusEntry};
use agent_client_protocol::{
    self as acp, ExtNotification, SessionConfigKind, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigSelectOptions, SessionUpdate,
};
use crossterm::event::{self, KeyCode, KeyEvent};
use tokio::sync::oneshot;
use utils::ReasoningEffort;

pub enum AppAction {
    Key(KeyEvent),
    Paste(String),
    Resize {
        cols: u16,
        rows: u16,
    },
    Tick,
    SessionUpdate(SessionUpdate),
    ExtNotification(ExtNotification),
    PromptDone,
    PromptError,
    ElicitationRequest {
        params: ElicitationParams,
        response_tx: oneshot::Sender<ElicitationResponse>,
    },
    AuthenticateComplete {
        method_id: String,
    },
    AuthenticateFailed {
        method_id: String,
        error: String,
    },
    /// Test-only: inject file picker matches via dispatch instead of reaching into internal state.
    #[doc(hidden)]
    #[allow(dead_code)]
    SetFilePickerMatches(Vec<crate::components::file_picker::FileMatch>),
}

pub struct UiState {
    pub(crate) tool_call_statuses: ToolCallStatuses,
    pub(crate) grid_loader: Spinner,
    pub(crate) conversation: ConversationBuffer,
    pub(crate) prompt_composer: PromptComposer,
    pub(crate) agent_name: String,
    pub(crate) mode_display: Option<String>,
    pub(crate) model_display: Option<String>,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    pub(crate) config_options: Vec<SessionConfigOption>,
    pub(crate) waiting_for_response: bool,
    pub(crate) context_usage_pct: Option<u8>,
    pub(crate) progress_indicator: ProgressIndicator,
    pub(crate) config_overlay: Option<ConfigOverlay>,
    pub(crate) elicitation_form: Option<ElicitationForm>,
    pub(crate) server_statuses: Vec<McpServerStatusEntry>,
    pub(crate) auth_methods: Vec<acp::AuthMethod>,
    pub(crate) plan_tracker: PlanTracker,
}

impl UiState {
    pub fn new(
        agent_name: String,
        config_options: &[SessionConfigOption],
        auth_methods: Vec<acp::AuthMethod>,
    ) -> Self {
        Self {
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: Spinner::default(),
            conversation: ConversationBuffer::new(),
            prompt_composer: PromptComposer::new(),
            agent_name,
            mode_display: extract_mode_display(config_options),
            model_display: extract_model_display(config_options),
            reasoning_effort: extract_reasoning_effort(config_options),
            config_options: config_options.to_vec(),
            waiting_for_response: false,
            context_usage_pct: None,
            progress_indicator: ProgressIndicator::default(),
            config_overlay: None,
            elicitation_form: None,
            server_statuses: Vec::new(),
            auth_methods,
            plan_tracker: PlanTracker::default(),
        }
    }

    pub(crate) fn on_key_event(&mut self, key_event: KeyEvent) -> Vec<AppEffect> {
        if key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            return vec![AppEffect::Exit];
        }

        if let Some(effects) = self.handle_elicitation_key(key_event) {
            return effects;
        }

        if let Some(ref mut overlay) = self.config_overlay {
            let outcome = overlay.on_key_event(key_event);
            return self.handle_config_overlay_outcome(outcome);
        }

        let composer_outcome = self.prompt_composer.on_key_event(key_event);
        if composer_outcome.consumed {
            return self.handle_prompt_composer_outcome(composer_outcome);
        }

        if key_event.code == KeyCode::BackTab {
            if let Some(effect) = self.cycle_quick_option() {
                return vec![effect, AppEffect::Render];
            }
            return vec![];
        }

        if key_event.code == KeyCode::Char('t')
            && key_event.modifiers.contains(event::KeyModifiers::ALT)
        {
            if let Some(effect) = self.cycle_reasoning_option() {
                return vec![effect, AppEffect::Render];
            }
            return vec![];
        }

        if key_event.code == KeyCode::Esc && self.waiting_for_response {
            return vec![AppEffect::Cancel];
        }

        vec![]
    }

    pub(crate) fn on_paste(&mut self, text: String) -> Vec<AppEffect> {
        self.config_overlay = None;
        if self.prompt_composer.on_paste(&text) {
            vec![AppEffect::Render]
        } else {
            vec![]
        }
    }

    pub(crate) fn on_resize(&mut self, _cols: u16, _rows: u16) -> Vec<AppEffect> {
        vec![AppEffect::Render]
    }

    pub(crate) fn update_config_options(&mut self, config_options: &[SessionConfigOption]) {
        self.mode_display = extract_mode_display(config_options);
        self.model_display = extract_model_display(config_options);
        self.reasoning_effort = extract_reasoning_effort(config_options);
        self.config_options = config_options.to_vec();
    }

    pub(crate) fn cycle_quick_option(&self) -> Option<AppEffect> {
        let option = self
            .config_options
            .iter()
            .find(|option| is_cycleable_mode_option(option))?;

        let SessionConfigKind::Select(ref select) = option.kind else {
            return None;
        };

        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            return None;
        };

        if options.is_empty() {
            return None;
        }

        let current_index = options
            .iter()
            .position(|entry| entry.value == select.current_value)
            .unwrap_or(0);
        let next_index = (current_index + 1) % options.len();
        let next_value = options.get(next_index)?.value.0.to_string();

        Some(AppEffect::SetConfigOption {
            config_id: option.id.0.to_string(),
            new_value: next_value,
        })
    }

    pub(crate) fn cycle_reasoning_option(&self) -> Option<AppEffect> {
        self.config_options
            .iter()
            .any(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())
            .then(|| {
                let next = ReasoningEffort::cycle_next(self.reasoning_effort);
                AppEffect::SetConfigOption {
                    config_id: ConfigOptionId::ReasoningEffort.as_str().to_string(),
                    new_value: ReasoningEffort::config_str(next).to_string(),
                }
            })
    }

    pub(crate) fn handle_elicitation_key(&mut self, key_event: KeyEvent) -> Option<Vec<AppEffect>> {
        let elicitation_form = self.elicitation_form.as_mut()?;
        let outcome = elicitation_form.form.on_key_event(key_event);

        match outcome.action {
            Some(FormAction::Close) => {
                if let Some(elicitation_form) = self.elicitation_form.take() {
                    let _ = elicitation_form
                        .response_tx
                        .send(ElicitationForm::decline());
                }
            }
            Some(FormAction::Submit) => {
                if let Some(elicitation_form) = self.elicitation_form.take() {
                    let response = elicitation_form.confirm();
                    let _ = elicitation_form.response_tx.send(response);
                }
            }
            None => {}
        }

        if outcome.needs_render {
            Some(vec![AppEffect::Render])
        } else {
            Some(vec![])
        }
    }

    pub(crate) fn handle_prompt_composer_outcome(
        &mut self,
        outcome: InputOutcome<PromptComposerAction>,
    ) -> Vec<AppEffect> {
        match outcome.action {
            Some(PromptComposerAction::OpenConfig) => {
                self.open_config_overlay();
                vec![AppEffect::Render]
            }
            Some(PromptComposerAction::SubmitRequested {
                user_input,
                attachments,
            }) => {
                let mut effects = vec![
                    AppEffect::PushScrollback(vec![Line::new(String::new())]),
                    AppEffect::PushScrollback(vec![Line::new(user_input.clone())]),
                ];

                self.waiting_for_response = true;
                self.grid_loader.reset();

                effects.push(AppEffect::Render);
                effects.push(AppEffect::PromptSubmit {
                    user_input,
                    attachments,
                });
                effects
            }
            None if outcome.needs_render => vec![AppEffect::Render],
            None => vec![],
        }
    }

    pub(crate) fn handle_config_overlay_outcome(
        &mut self,
        outcome: InputOutcome<ConfigOverlayAction>,
    ) -> Vec<AppEffect> {
        match outcome.action {
            Some(ConfigOverlayAction::Close) => {
                self.config_overlay = None;
                vec![AppEffect::Render]
            }
            Some(ConfigOverlayAction::ApplyConfigChanges(changes)) => {
                let mut effects = Vec::new();
                for change in changes {
                    if change.config_id == THEME_CONFIG_ID {
                        effects.push(AppEffect::SetTheme {
                            file: theme_file_from_picker_value(&change.new_value),
                        });
                    } else {
                        effects.push(AppEffect::SetConfigOption {
                            config_id: change.config_id,
                            new_value: change.new_value,
                        });
                    }
                }
                effects.push(AppEffect::Render);
                effects
            }
            Some(ConfigOverlayAction::AuthenticateServer(name)) => {
                vec![
                    AppEffect::AuthenticateMcpServer { server_name: name },
                    AppEffect::Render,
                ]
            }
            Some(ConfigOverlayAction::AuthenticateProvider(method_id)) => {
                vec![
                    AppEffect::AuthenticateProvider { method_id },
                    AppEffect::Render,
                ]
            }
            None if outcome.needs_render => vec![AppEffect::Render],
            None => vec![],
        }
    }

    pub(crate) fn open_config_overlay(&mut self) {
        let menu = ConfigMenu::from_config_options(&self.config_options);
        let menu = self.decorate_config_menu(menu);
        self.config_overlay = Some(
            ConfigOverlay::new(
                menu,
                self.server_statuses.clone(),
                self.auth_methods.clone(),
            )
            .with_reasoning_effort_from_options(&self.config_options),
        );
    }

    pub(crate) fn decorate_config_menu(&self, mut menu: ConfigMenu) -> ConfigMenu {
        let settings = load_or_create_settings();
        let theme_files = list_theme_files();
        menu.add_theme_entry(settings.theme.file.as_deref(), &theme_files);

        let server_summary = server_status_summary(&self.server_statuses);
        menu.add_mcp_servers_entry(&server_summary);
        if !self.auth_methods.is_empty() {
            let summary = format!("{} needs login", self.auth_methods.len());
            menu.add_provider_logins_entry(&summary);
        }
        menu
    }

    #[cfg(test)]
    pub(crate) fn available_commands(&self) -> &[crate::components::command_picker::CommandEntry] {
        self.prompt_composer.available_commands()
    }

    pub(crate) fn on_authenticate_started(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_started(method_id);
        }
    }
}

pub(super) fn is_cycleable_mode_option(option: &SessionConfigOption) -> bool {
    matches!(option.kind, SessionConfigKind::Select(_))
        && option.category == Some(SessionConfigOptionCategory::Mode)
}

pub(super) fn option_display_name(
    options: &SessionConfigSelectOptions,
    current_value: &acp::SessionConfigValueId,
) -> Option<String> {
    match options {
        SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .find(|option| &option.value == current_value)
            .map(|option| option.name.clone()),
        SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| group.options.iter())
            .find(|option| &option.value == current_value)
            .map(|option| option.name.clone()),
        _ => None,
    }
}

pub(super) fn extract_select_display(
    config_options: &[SessionConfigOption],
    id: ConfigOptionId,
) -> Option<String> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == id.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    option_display_name(&select.options, &select.current_value)
}

pub(super) fn extract_mode_display(config_options: &[SessionConfigOption]) -> Option<String> {
    extract_select_display(config_options, ConfigOptionId::Mode)
}

pub(super) fn extract_model_display(config_options: &[SessionConfigOption]) -> Option<String> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == ConfigOptionId::Model.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    let options = match &select.options {
        SessionConfigSelectOptions::Ungrouped(options) => options,
        SessionConfigSelectOptions::Grouped(_) => {
            return extract_select_display(config_options, ConfigOptionId::Model);
        }
        _ => return None,
    };

    let current = select.current_value.0.as_ref();
    if current.contains(',') {
        let names: Vec<&str> = current
            .split(',')
            .filter_map(|part| {
                let trimmed = part.trim();
                options
                    .iter()
                    .find(|option| option.value.0.as_ref() == trimmed)
                    .map(|option| option.name.as_str())
            })
            .collect();
        if names.is_empty() {
            None
        } else {
            Some(names.join(" + "))
        }
    } else {
        extract_select_display(config_options, ConfigOptionId::Model)
    }
}

pub(super) fn extract_reasoning_effort(
    config_options: &[SessionConfigOption],
) -> Option<ReasoningEffort> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    ReasoningEffort::parse(&select.current_value.0).unwrap_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::SessionConfigOptionCategory;

    #[test]
    fn ui_state_new_initializes_derived_displays() {
        let config_options = vec![
            SessionConfigOption::select(
                "mode",
                "Mode",
                "planner",
                vec![
                    acp::SessionConfigSelectOption::new("planner", "Planner"),
                    acp::SessionConfigSelectOption::new("coder", "Coder"),
                ],
            )
            .category(SessionConfigOptionCategory::Mode),
            SessionConfigOption::select(
                "model",
                "Model",
                "a:x,b:y",
                vec![
                    acp::SessionConfigSelectOption::new("a:x", "Alpha / X"),
                    acp::SessionConfigSelectOption::new("b:y", "Beta / Y"),
                ],
            )
            .category(SessionConfigOptionCategory::Model),
            SessionConfigOption::select(
                ConfigOptionId::ReasoningEffort.as_str(),
                "Reasoning",
                "high",
                vec![
                    acp::SessionConfigSelectOption::new("none", "None"),
                    acp::SessionConfigSelectOption::new("high", "High"),
                ],
            ),
        ];

        let state = UiState::new(
            "test-agent".to_string(),
            &config_options,
            vec![acp::AuthMethod::new("anthropic", "Anthropic")],
        );

        assert_eq!(state.agent_name, "test-agent");
        assert_eq!(state.mode_display.as_deref(), Some("Planner"));
        assert_eq!(state.model_display.as_deref(), Some("Alpha / X + Beta / Y"));
        assert_eq!(state.reasoning_effort, Some(ReasoningEffort::High));
        assert_eq!(state.config_options.len(), 3);
        assert_eq!(state.auth_methods.len(), 1);
    }

    #[test]
    fn ui_state_new_initializes_defaults() {
        let state = UiState::new("test-agent".to_string(), &[], vec![]);

        assert!(!state.waiting_for_response);
        assert_eq!(state.context_usage_pct, None);
        assert!(state.config_overlay.is_none());
        assert!(state.elicitation_form.is_none());
        assert!(state.server_statuses.is_empty());
    }
}

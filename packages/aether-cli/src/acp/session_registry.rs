use super::config_setting::ConfigSetting;
use super::model_config::{
    ValidatedMode, effective_model, mode_name_for_state_from_modes, model_exists, resolve_mode_from_modes,
};
use super::relay::{RelayHandle, SessionCommand};
use acp_utils::config_option_id::ConfigOptionId;
use acp_utils::notifications::McpRequest;
use agent_client_protocol::schema as acp;
use llm::ReasoningEffort;
use llm::catalog::LlmModel;
use std::collections::HashMap;
use tokio::sync::{Mutex, mpsc};
use tracing::error;

/// Owns the live session map for an ACP agent.
///
/// Exposes concrete operations rather than closure-scoped field access so
/// callers never touch per-session state directly and broadcast fanout never
/// holds the registry lock across I/O.
pub(crate) struct SessionRegistry {
    sessions: Mutex<HashMap<String, SessionState>>,
}

#[derive(Clone)]
pub(crate) struct RelayHandles {
    pub cmd: mpsc::Sender<SessionCommand>,
    pub mcp_request: mpsc::Sender<McpRequest>,
}

pub(crate) struct PromptDispatch {
    pub relay_tx: mpsc::Sender<SessionCommand>,
    pub switch_model: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Clone)]
pub(crate) struct ConfigSnapshot {
    pub modes: Vec<ValidatedMode>,
    pub selected_mode: Option<String>,
    pub effective_model: String,
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }

    /// Register a new session's relay and initial config.
    pub async fn insert(
        &self,
        id: String,
        relay: RelayHandle,
        active_model: String,
        selected_mode: Option<String>,
        reasoning_effort: Option<ReasoningEffort>,
        modes: Vec<ValidatedMode>,
    ) {
        let state = SessionState::new(relay, active_model, selected_mode, reasoning_effort, modes);
        self.sessions.lock().await.insert(id, state);
    }

    /// Clone the relay channels for a session.
    pub async fn relay(&self, id: &str) -> Option<RelayHandles> {
        self.sessions.lock().await.get(id).map(|state| RelayHandles {
            cmd: state.relay.cmd_tx.clone(),
            mcp_request: state.relay.mcp_request_tx.clone(),
        })
    }

    /// Effective model string (pending if set, otherwise active). No mutation.
    pub async fn effective_model(&self, id: &str) -> Option<String> {
        self.sessions
            .lock()
            .await
            .get(id)
            .map(|state| effective_model(&state.config.active_model, state.config.pending_model.as_deref()).to_string())
    }

    /// Commit any pending model switch and return the dispatch info a prompt needs.
    pub async fn begin_prompt(&self, id: &str) -> Option<PromptDispatch> {
        let mut sessions = self.sessions.lock().await;
        let state = sessions.get_mut(id)?;

        let switch_model = state.config.pending_model.take().and_then(|pending| {
            if pending == state.config.active_model {
                None
            } else {
                state.config.active_model.clone_from(&pending);
                Some(pending)
            }
        });

        Some(PromptDispatch {
            relay_tx: state.relay.cmd_tx.clone(),
            switch_model,
            reasoning_effort: state.config.reasoning_effort,
        })
    }

    /// Apply a config change and return the session's updated snapshot. Returns
    /// `None` if the session is unknown, `Some(Err)` if the change is invalid.
    pub async fn apply_config_change(
        &self,
        id: &str,
        setting: &ConfigSetting,
        available: &[LlmModel],
    ) -> Option<Result<ConfigSnapshot, acp::Error>> {
        let mut sessions = self.sessions.lock().await;
        let state = sessions.get_mut(id)?;
        Some(state.config.apply_config_change(&state.modes, available, setting).map(|()| state.snapshot()))
    }

    /// Clone every session's config snapshot for broadcast fanout. The lock is
    /// released before the caller sends notifications.
    pub async fn snapshot_all_configs(&self) -> Vec<(String, ConfigSnapshot)> {
        let sessions = self.sessions.lock().await;
        sessions.iter().map(|(id, state)| (id.clone(), state.snapshot())).collect()
    }

    /// Drain every session and stop its relay task. Blocks until every relay has exited.
    pub async fn shutdown_all(&self) {
        let relays: Vec<RelayHandle> = {
            let mut sessions = self.sessions.lock().await;
            sessions.drain().map(|(_, state)| state.relay).collect()
        };

        futures::future::join_all(relays.into_iter().map(RelayHandle::stop)).await;
    }
}

struct SessionState {
    relay: RelayHandle,
    config: SessionConfigState,
    modes: Vec<ValidatedMode>,
}

impl SessionState {
    fn new(
        relay: RelayHandle,
        active_model: String,
        selected_mode: Option<String>,
        reasoning_effort: Option<ReasoningEffort>,
        modes: Vec<ValidatedMode>,
    ) -> Self {
        let mut config = SessionConfigState::new(active_model);
        config.reasoning_effort = reasoning_effort;
        config.selected_mode = selected_mode;
        Self { relay, config, modes }
    }

    fn snapshot(&self) -> ConfigSnapshot {
        ConfigSnapshot {
            modes: self.modes.clone(),
            selected_mode: self.config.selected_mode.clone(),
            effective_model: effective_model(&self.config.active_model, self.config.pending_model.as_deref())
                .to_string(),
            reasoning_effort: self.config.reasoning_effort,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SessionConfigState {
    active_model: String,
    pending_model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    selected_mode: Option<String>,
}

impl SessionConfigState {
    fn new(active_model: String) -> Self {
        Self { active_model, pending_model: None, reasoning_effort: None, selected_mode: None }
    }

    fn apply_config_change(
        &mut self,
        validated_modes: &[ValidatedMode],
        available: &[LlmModel],
        setting: &ConfigSetting,
    ) -> Result<(), acp::Error> {
        match setting {
            ConfigSetting::Mode(value) => {
                let Some((mode_model, mode_reasoning_effort)) = resolve_mode_from_modes(validated_modes, value) else {
                    error!("Unknown or invalid mode: {}", value);
                    return Err(acp::Error::invalid_params());
                };

                self.pending_model = (self.active_model != mode_model).then_some(mode_model);
                self.reasoning_effort = mode_reasoning_effort;
                self.selected_mode = Some(value.clone());
            }
            ConfigSetting::Model(value) => {
                if !model_exists(available, value) {
                    error!("Unknown model in set_session_config_option: {}", value);
                    return Err(acp::Error::invalid_params());
                }
                self.pending_model = (self.active_model != *value).then_some(value.clone());
            }
            ConfigSetting::ReasoningEffort(effort) => {
                self.reasoning_effort = *effort;
            }
        }

        let effective = effective_model(&self.active_model, self.pending_model.as_deref());
        if setting.config_id() == ConfigOptionId::Model {
            self.selected_mode = mode_name_for_state_from_modes(validated_modes, effective, self.reasoning_effort);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ReasoningEffort as RE;

    const SONNET: &str = "anthropic:claude-sonnet-4-5";
    const DEEPSEEK: &str = "deepseek:deepseek-chat";

    fn available_models() -> Vec<LlmModel> {
        [SONNET, "anthropic:claude-opus-4-6", DEEPSEEK].into_iter().map(|s| s.parse().expect("valid model")).collect()
    }

    fn validated_modes() -> Vec<ValidatedMode> {
        let m = |name: &str, model: &str, effort| ValidatedMode {
            name: name.into(),
            model: model.into(),
            reasoning_effort: effort,
        };
        vec![m("Planner", SONNET, Some(RE::High)), m("Coder", DEEPSEEK, None)]
    }

    fn apply(
        active: &str,
        effort: Option<RE>,
        mode: Option<&str>,
        setting: &ConfigSetting,
    ) -> (Result<(), acp::Error>, SessionConfigState) {
        let mut state = SessionConfigState::new(active.into());
        state.reasoning_effort = effort;
        state.selected_mode = mode.map(Into::into);
        let result = state.apply_config_change(&validated_modes(), &available_models(), setting);
        (result, state)
    }

    #[test]
    fn new_state_has_no_pending_model_or_mode() {
        let s = SessionConfigState::new(DEEPSEEK.into());
        assert!((s.pending_model.is_none() && s.reasoning_effort.is_none() && s.selected_mode.is_none()));
    }

    #[test]
    fn mode_selection_updates_pending_model_and_reasoning() {
        let (res, s) = apply(DEEPSEEK, None, None, &ConfigSetting::Mode("Planner".into()));
        assert!(res.is_ok());
        assert_eq!(s.pending_model.as_deref(), Some(SONNET));
        assert_eq!(s.reasoning_effort, Some(RE::High));
        assert_eq!(s.selected_mode.as_deref(), Some("Planner"));
    }

    #[test]
    fn unknown_mode_is_rejected() {
        let (res, _) = apply(DEEPSEEK, None, None, &ConfigSetting::Mode("Unknown".into()));
        assert!(res.is_err());
    }

    #[test]
    fn effort_change_preserves_mode_and_model_change_clears_it() {
        let (res, s) = apply(SONNET, Some(RE::High), Some("Planner"), &ConfigSetting::ReasoningEffort(Some(RE::Low)));
        assert!(res.is_ok());
        assert_eq!(s.reasoning_effort, Some(RE::Low));
        assert_eq!(s.selected_mode.as_deref(), Some("Planner"));

        let (res, s) = apply(SONNET, Some(RE::Medium), Some("Planner"), &ConfigSetting::Model(DEEPSEEK.into()));
        assert!(res.is_ok());
        assert_eq!(s.pending_model.as_deref(), Some(DEEPSEEK));
        assert!(s.selected_mode.is_none());
    }
}

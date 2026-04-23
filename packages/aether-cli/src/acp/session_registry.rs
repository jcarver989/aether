use acp_utils::config_option_id::ConfigOptionId;
use agent_client_protocol::schema as acp;
use llm::ReasoningEffort;
use llm::catalog::LlmModel;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tracing::error;

use super::config_setting::ConfigSetting;
use super::model_config::{
    ValidatedMode, effective_model, mode_name_for_state_from_modes, model_exists, resolve_mode_from_modes,
};
use super::relay::RelayHandle;

/// Owns the live session map and the relay-lifecycle side of shutdown.
///
/// Access to session state is scoped via closure-based `with` / `with_mut`
/// helpers so callers cannot forget to release the lock or leak a reference.
pub(crate) struct SessionRegistry {
    sessions: Mutex<HashMap<String, SessionState>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }

    pub async fn insert(&self, id: String, state: SessionState) {
        self.sessions.lock().await.insert(id, state);
    }

    /// Run `f` against a session's mutable state. Returns `None` if the id is unknown.
    pub async fn with_mut<R>(&self, id: &str, f: impl FnOnce(&mut SessionState) -> R) -> Option<R> {
        let mut sessions = self.sessions.lock().await;
        sessions.get_mut(id).map(f)
    }

    /// Run `f` against a session's shared state. Returns `None` if the id is unknown.
    pub async fn with<R>(&self, id: &str, f: impl FnOnce(&SessionState) -> R) -> Option<R> {
        let sessions = self.sessions.lock().await;
        sessions.get(id).map(f)
    }

    /// Visit every session while holding the lock. Callers must not do anything
    /// that suspends on an `await` inside the closure.
    pub async fn for_each(&self, mut f: impl FnMut(&str, &SessionState)) {
        let sessions = self.sessions.lock().await;
        for (id, state) in sessions.iter() {
            f(id, state);
        }
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

/// Per-session state including active and staged model selections.
pub(crate) struct SessionState {
    pub relay: RelayHandle,
    pub config: SessionConfigState,
    pub modes: Vec<ValidatedMode>,
}

impl SessionState {
    pub fn new(
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
}

/// Mutable per-session config state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SessionConfigState {
    pub active_model: String,
    pub pending_model: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub selected_mode: Option<String>,
}

impl SessionConfigState {
    pub fn new(active_model: String) -> Self {
        Self { active_model, pending_model: None, reasoning_effort: None, selected_mode: None }
    }

    pub fn apply_config_change(
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

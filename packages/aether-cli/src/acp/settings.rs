use acp_utils::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AetherCliSettings {
    #[serde(default)]
    pub modes: BTreeMap<String, Mode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    pub model: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
}

pub fn load_or_create_settings() -> AetherCliSettings {
    if let Some(store) = SettingsStore::new("AETHER_HOME", ".aether") {
        store.load_or_create()
    } else {
        warn!("Unable to resolve Aether settings path; using defaults");
        AetherCliSettings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_utils::settings::SettingsStore;
    use tempfile::TempDir;

    #[test]
    fn round_trip_serde() {
        let temp_dir = TempDir::new().unwrap();
        let store = SettingsStore::from_path(temp_dir.path());
        let mut modes = BTreeMap::new();
        modes.insert(
            "Planner".to_string(),
            Mode {
                model: "codex:gpt-5.3".to_string(),
                reasoning_effort: Some("high".to_string()),
            },
        );
        let settings = AetherCliSettings { modes };

        store.save(&settings).unwrap();
        let loaded: AetherCliSettings = store.load_or_create();

        assert_eq!(loaded, settings);
    }
}

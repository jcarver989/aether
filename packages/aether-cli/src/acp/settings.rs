use acp_utils::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AetherCliSettings {
    #[serde(default)]
    pub modes: Vec<Mode>,
    #[serde(default = "default_prompts")]
    pub prompts: Vec<String>,
}

impl Default for AetherCliSettings {
    fn default() -> Self {
        Self {
            modes: Vec::new(),
            prompts: default_prompts(),
        }
    }
}

impl AetherCliSettings {
    /// Merge project-level settings into self (user-level).
    /// - `prompts`: concatenated (user first, then project, deduplicated)
    /// - `modes`: project overrides user for same names, new modes appended
    pub fn merge(&mut self, other: AetherCliSettings) {
        for mode in other.modes {
            if let Some(existing) = self.modes.iter_mut().find(|m| m.name == mode.name) {
                *existing = mode;
            } else {
                self.modes.push(mode);
            }
        }

        for pattern in other.prompts {
            if !self.prompts.contains(&pattern) {
                self.prompts.push(pattern);
            }
        }
    }
}

fn default_prompts() -> Vec<String> {
    vec!["AGENTS.md".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    pub name: String,
    pub model: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
}

pub fn load_or_create_settings(project_dir: Option<&Path>) -> AetherCliSettings {
    let mut settings = if let Some(store) = SettingsStore::new("AETHER_HOME", ".aether") {
        store.load_or_create()
    } else {
        warn!("Unable to resolve Aether settings path; using defaults");
        AetherCliSettings::default()
    };

    if let Some(project) = project_dir.and_then(load_project_settings) {
        settings.merge(project);
    }

    settings
}

fn load_project_settings(dir: &Path) -> Option<AetherCliSettings> {
    let path = dir.join(".aether/settings.json");
    let raw = fs::read_to_string(&path)
        .inspect_err(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                warn!("Failed to read project settings at {}: {e}", path.display());
            }
        })
        .ok()?;
    serde_json::from_str(&raw)
        .inspect_err(|e| warn!("Malformed project settings at {}: {e}", path.display()))
        .ok()
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
        let settings = AetherCliSettings {
            modes: vec![Mode {
                name: "Planner".to_string(),
                model: "codex:gpt-5.3".to_string(),
                reasoning_effort: Some("high".to_string()),
            }],
            prompts: vec!["AGENTS.md".to_string()],
        };

        store.save(&settings).unwrap();
        let loaded: AetherCliSettings = store.load_or_create();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn default_prompts_includes_agents_md() {
        let settings = AetherCliSettings::default();
        assert_eq!(settings.prompts, vec!["AGENTS.md".to_string()]);
    }

    #[test]
    fn merge_concatenates_prompts() {
        let mut user = AetherCliSettings {
            prompts: vec!["AGENTS.md".to_string()],
            ..Default::default()
        };
        let project = AetherCliSettings {
            prompts: vec![".aether/rules/*.md".to_string()],
            ..Default::default()
        };

        user.merge(project);
        assert_eq!(
            user.prompts,
            vec!["AGENTS.md".to_string(), ".aether/rules/*.md".to_string()]
        );
    }

    #[test]
    fn merge_deduplicates_prompts() {
        let mut user = AetherCliSettings {
            prompts: vec!["AGENTS.md".to_string()],
            ..Default::default()
        };
        let project = AetherCliSettings {
            prompts: vec!["AGENTS.md".to_string(), "SYSTEM.md".to_string()],
            ..Default::default()
        };

        user.merge(project);
        assert_eq!(
            user.prompts,
            vec!["AGENTS.md".to_string(), "SYSTEM.md".to_string()]
        );
    }

    #[test]
    fn merge_modes_project_overrides() {
        let mut user = AetherCliSettings::default();
        user.modes.push(Mode {
            name: "Fast".to_string(),
            model: "user-model".to_string(),
            reasoning_effort: None,
        });

        let mut project = AetherCliSettings::default();
        project.modes.push(Mode {
            name: "Fast".to_string(),
            model: "project-model".to_string(),
            reasoning_effort: Some("high".to_string()),
        });

        user.merge(project);
        assert_eq!(user.modes[0].model, "project-model");
    }

    #[test]
    fn load_merges_project_settings() {
        let project_dir = TempDir::new().unwrap();
        let aether_dir = project_dir.path().join(".aether");
        std::fs::create_dir_all(&aether_dir).unwrap();
        std::fs::write(
            aether_dir.join("settings.json"),
            r#"{"prompts": [".aether/rules/*.md"]}"#,
        )
        .unwrap();

        let settings = load_or_create_settings(Some(project_dir.path()));
        assert!(settings.prompts.contains(&"AGENTS.md".to_string()));
        assert!(settings.prompts.contains(&".aether/rules/*.md".to_string()));
    }

    #[test]
    fn load_ignores_project_session_persistence_setting() {
        let project_dir = TempDir::new().unwrap();
        let aether_dir = project_dir.path().join(".aether");
        std::fs::create_dir_all(&aether_dir).unwrap();
        std::fs::write(
            aether_dir.join("settings.json"),
            r#"{"sessionPersistence": false}"#,
        )
        .unwrap();

        let baseline = load_or_create_settings(None);
        let settings = load_or_create_settings(Some(project_dir.path()));
        assert_eq!(settings, baseline);
    }

    #[test]
    fn round_trip_with_prompts() {
        let temp_dir = TempDir::new().unwrap();
        let store = SettingsStore::from_path(temp_dir.path());
        let settings = AetherCliSettings {
            modes: Vec::new(),
            prompts: vec![
                "AGENTS.md".to_string(),
                ".aether/rules/*.md".to_string(),
                "/home/user/shared/coding.md".to_string(),
            ],
        };

        store.save(&settings).unwrap();
        let loaded: AetherCliSettings = store.load_or_create();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn modes_order_preserved_from_json() {
        let json = r#"{"modes": [{"name": "Zebra", "model": "model-z"}, {"name": "Alpha", "model": "model-a"}, {"name": "Middle", "model": "model-m"}], "prompts": []}"#;
        let settings: AetherCliSettings = serde_json::from_str(json).unwrap();
        let mode_names: Vec<_> = settings.modes.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            mode_names,
            vec!["Zebra", "Alpha", "Middle"],
            "modes should preserve JSON array order"
        );
    }
}

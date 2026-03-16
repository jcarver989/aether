pub mod menu;
pub mod overlay;
pub(crate) mod picker;
pub mod types;

use crate::components::provider_login::{
    ProviderLoginEntry, ProviderLoginStatus, provider_login_summary,
};
use crate::components::server_status::server_status_summary;
use acp_utils::notifications::McpServerStatusEntry;
use acp_utils::settings::SettingsStore;
use agent_client_protocol::AuthMethod;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;

#[cfg(test)]
pub(crate) static WISP_HOME_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WispSettings {
    #[serde(default)]
    pub theme: ThemeSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettings {
    #[serde(default)]
    pub file: Option<String>,
}

pub fn wisp_home() -> Option<PathBuf> {
    Some(
        SettingsStore::new("WISP_HOME", ".wisp")?
            .home()
            .to_path_buf(),
    )
}

pub fn themes_dir_path() -> Option<PathBuf> {
    Some(wisp_home()?.join("themes"))
}

pub fn load_or_create_settings() -> WispSettings {
    if let Some(store) = SettingsStore::new("WISP_HOME", ".wisp") {
        store.load_or_create()
    } else {
        warn!("Unable to resolve Wisp settings path; using defaults");
        WispSettings::default()
    }
}

pub fn load_theme(settings: &WispSettings) -> tui::Theme {
    let Some(theme_file) = settings.theme.file.as_deref() else {
        return tui::Theme::default();
    };

    let Some(path) = resolve_theme_file_path(theme_file) else {
        warn!("Rejected unsafe theme filename: {}", theme_file);
        return tui::Theme::default();
    };

    tui::Theme::load_from_path(&path)
}

pub fn resolve_theme_file_path(file_name: &str) -> Option<PathBuf> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = Path::new(trimmed);
    let base_name = candidate.file_name()?.to_str()?;
    if base_name != trimmed {
        return None;
    }

    if base_name == "." || base_name == ".." {
        return None;
    }

    Some(themes_dir_path()?.join(base_name))
}

pub fn list_theme_files() -> Vec<String> {
    let Some(themes_dir) = themes_dir_path() else {
        return Vec::new();
    };

    let Ok(entries) = std::fs::read_dir(themes_dir) else {
        return Vec::new();
    };

    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let Ok(file_type) = entry.file_type() else {
                return None;
            };

            if !file_type.is_file() {
                return None;
            }

            let name = entry.file_name().into_string().ok()?;
            if !name.ends_with(".tmTheme") {
                return None;
            }
            Some(name)
        })
        .collect::<Vec<_>>();

    files.sort_unstable();
    files
}

pub(crate) fn build_login_entries(auth_methods: &[AuthMethod]) -> Vec<ProviderLoginEntry> {
    auth_methods
        .iter()
        .map(|m| {
            let status = if m.description() == Some("authenticated") {
                ProviderLoginStatus::LoggedIn
            } else {
                ProviderLoginStatus::NeedsLogin
            };
            ProviderLoginEntry {
                method_id: m.id().0.to_string(),
                name: m.name().to_string(),
                status,
            }
        })
        .collect()
}

pub(crate) fn create_overlay(
    config_options: &[agent_client_protocol::SessionConfigOption],
    server_statuses: &[McpServerStatusEntry],
    auth_methods: &[agent_client_protocol::AuthMethod],
) -> overlay::SettingsOverlay {
    let mut menu = menu::SettingsMenu::from_config_options(config_options);
    decorate_menu(&mut menu, server_statuses, auth_methods);
    overlay::SettingsOverlay::new(menu, server_statuses.to_vec(), auth_methods.to_vec())
        .with_reasoning_effort_from_options(config_options)
}

pub(crate) fn decorate_menu(
    menu: &mut menu::SettingsMenu,
    server_statuses: &[McpServerStatusEntry],
    auth_methods: &[AuthMethod],
) {
    let settings = load_or_create_settings();
    let theme_files = list_theme_files();
    menu.add_theme_entry(settings.theme.file.as_deref(), &theme_files);

    let summary = server_status_summary(server_statuses);
    menu.add_mcp_servers_entry(&summary);

    if !auth_methods.is_empty() {
        let login_entries = build_login_entries(auth_methods);
        let login_summary = provider_login_summary(&login_entries);
        menu.add_provider_logins_entry(&login_summary);
    }
}

pub(crate) fn process_config_changes(
    changes: Vec<types::SettingsChange>,
) -> Vec<overlay::SettingsMessage> {
    use acp_utils::config_option_id::THEME_CONFIG_ID;

    let mut messages = Vec::new();
    for change in changes {
        if change.config_id == THEME_CONFIG_ID {
            let file = theme_file_from_picker_value(&change.new_value);
            let mut settings = load_or_create_settings();
            settings.theme.file = file;
            if let Err(err) = save_settings(&settings) {
                tracing::warn!("Failed to persist theme setting: {err}");
            }
            let theme = load_theme(&settings);
            messages.push(overlay::SettingsMessage::SetTheme(theme));
        } else {
            messages.push(overlay::SettingsMessage::SetConfigOption {
                config_id: change.config_id,
                value: change.new_value,
            });
        }
    }
    messages
}

fn theme_file_from_picker_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn cycle_quick_option(
    config_options: &[agent_client_protocol::SessionConfigOption],
) -> Option<(String, String)> {
    use crate::components::status_line::is_cycleable_mode_option;
    use agent_client_protocol::{SessionConfigKind, SessionConfigSelectOptions};

    let option = config_options
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
    options
        .get(next_index)
        .map(|next| (option.id.0.to_string(), next.value.0.to_string()))
}

pub(crate) fn cycle_reasoning_option(
    config_options: &[agent_client_protocol::SessionConfigOption],
) -> Option<(String, String)> {
    use crate::components::status_line::extract_reasoning_effort;
    use acp_utils::config_option_id::ConfigOptionId;
    use utils::ReasoningEffort;

    let has_reasoning = config_options
        .iter()
        .any(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str());

    if has_reasoning {
        let current = extract_reasoning_effort(config_options);
        let next = ReasoningEffort::cycle_next(current);
        Some((
            ConfigOptionId::ReasoningEffort.as_str().to_string(),
            ReasoningEffort::config_str(next).to_string(),
        ))
    } else {
        None
    }
}

pub(crate) fn unhealthy_server_count(statuses: &[McpServerStatusEntry]) -> usize {
    use acp_utils::notifications::McpServerStatus;

    statuses
        .iter()
        .filter(|status| !matches!(status.status, McpServerStatus::Connected { .. }))
        .count()
}

pub fn save_settings(settings: &WispSettings) -> std::io::Result<()> {
    let store = SettingsStore::new("WISP_HOME", ".wisp").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Unable to resolve Wisp settings path",
        )
    })?;

    store.save(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_wisp_home;
    use acp_utils::settings::SettingsStore;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn round_trip_serde() {
        let temp_dir = TempDir::new().unwrap();
        let store = SettingsStore::from_path(temp_dir.path());
        let settings = WispSettings {
            theme: ThemeSettings {
                file: Some("my-theme.json".to_string()),
            },
        };

        store.save(&settings).unwrap();
        let loaded: WispSettings = store.load_or_create();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn resolve_theme_file_path_allows_basename_only() {
        assert!(resolve_theme_file_path("").is_none());
        assert!(resolve_theme_file_path("../escape.json").is_none());
        assert!(resolve_theme_file_path("subdir/theme.json").is_none());
        #[cfg(windows)]
        assert!(resolve_theme_file_path("..\\escape.json").is_none());
    }

    #[test]
    fn list_theme_files_returns_sorted_file_names() {
        let temp_dir = TempDir::new().unwrap();
        let themes = temp_dir.path().join("themes");
        fs::create_dir_all(&themes).unwrap();
        fs::write(themes.join("zeta.tmTheme"), "z").unwrap();
        fs::write(themes.join("alpha.tmTheme"), "a").unwrap();

        with_wisp_home(temp_dir.path(), || {
            let files = list_theme_files();
            assert_eq!(files, vec!["alpha.tmTheme", "zeta.tmTheme"]);
        });
    }

    #[test]
    fn list_theme_files_ignores_directories() {
        let temp_dir = TempDir::new().unwrap();
        let themes = temp_dir.path().join("themes");
        fs::create_dir_all(themes.join("nested")).unwrap();
        fs::write(themes.join("theme.tmTheme"), "ok").unwrap();

        with_wisp_home(temp_dir.path(), || {
            let files = list_theme_files();
            assert_eq!(files, vec!["theme.tmTheme"]);
        });
    }

    #[test]
    fn list_theme_files_returns_empty_when_themes_dir_missing() {
        let temp_dir = TempDir::new().unwrap();

        with_wisp_home(temp_dir.path(), || {
            let files = list_theme_files();
            assert!(files.is_empty());
        });
    }

    #[test]
    fn theme_default_value_maps_to_none() {
        assert_eq!(theme_file_from_picker_value("   "), None);
    }

    #[test]
    fn theme_value_maps_to_some() {
        assert_eq!(
            theme_file_from_picker_value("catppuccin.tmTheme"),
            Some("catppuccin.tmTheme".to_string())
        );
    }

    #[test]
    fn process_theme_change_persists_and_produces_set_theme() {
        use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
        use acp_utils::config_option_id::THEME_CONFIG_ID;
        use tui::Color;

        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("custom.tmTheme"), CUSTOM_TMTHEME).unwrap();

        with_wisp_home(temp_dir.path(), || {
            let messages = process_config_changes(vec![types::SettingsChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "custom.tmTheme".to_string(),
            }]);

            let theme_msg = messages.iter().find_map(|m| {
                if let overlay::SettingsMessage::SetTheme(theme) = m {
                    Some(theme)
                } else {
                    None
                }
            });
            assert!(theme_msg.is_some(), "should produce SetTheme message");
            assert_eq!(
                theme_msg.unwrap().text_primary(),
                Color::Rgb {
                    r: 0x11,
                    g: 0x22,
                    b: 0x33
                }
            );

            let loaded = load_or_create_settings();
            assert_eq!(loaded.theme.file.as_deref(), Some("custom.tmTheme"));
        });
    }

    #[test]
    fn process_theme_change_persists_default_as_none() {
        use crate::test_helpers::with_wisp_home;
        use acp_utils::config_option_id::THEME_CONFIG_ID;

        let temp_dir = TempDir::new().unwrap();
        with_wisp_home(temp_dir.path(), || {
            save_settings(&WispSettings {
                theme: ThemeSettings {
                    file: Some("old.tmTheme".to_string()),
                },
            })
            .unwrap();

            let _messages = process_config_changes(vec![types::SettingsChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "   ".to_string(),
            }]);

            let loaded = load_or_create_settings();
            assert_eq!(loaded.theme.file, None);
        });
    }

    #[test]
    fn process_non_theme_change_produces_set_setting_option() {
        let messages = process_config_changes(vec![types::SettingsChange {
            config_id: "provider".to_string(),
            new_value: "ollama".to_string(),
        }]);

        match messages.as_slice() {
            [overlay::SettingsMessage::SetConfigOption { config_id, value }] => {
                assert_eq!(config_id, "provider");
                assert_eq!(value, "ollama");
            }
            other => panic!("expected SetConfigOption, got: {other:?}"),
        }
    }
}

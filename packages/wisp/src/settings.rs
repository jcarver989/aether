use acp_utils::settings::SettingsStore;
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

pub fn load_theme(settings: &WispSettings) -> tui::theme::Theme {
    let Some(theme_file) = settings.theme.file.as_deref() else {
        return tui::theme::Theme::default();
    };

    let Some(path) = resolve_theme_file_path(theme_file) else {
        warn!("Rejected unsafe theme filename: {}", theme_file);
        return tui::theme::Theme::default();
    };

    tui::theme::Theme::load_from_path(&path)
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
    fn save_settings_persists_theme_file_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let settings = WispSettings {
            theme: ThemeSettings {
                file: Some("saved.tmTheme".to_string()),
            },
        };

        with_wisp_home(temp_dir.path(), || {
            save_settings(&settings).unwrap();
            let loaded = load_or_create_settings();
            assert_eq!(loaded.theme.file.as_deref(), Some("saved.tmTheme"));
        });
    }
}

use acp_utils::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;

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

#[cfg(test)]
mod tests {
    use super::*;
    use acp_utils::settings::SettingsStore;
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
}

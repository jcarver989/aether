//! Platform I/O abstractions.
//!
//! This module provides unified file system and path operations across platforms.
//!
//! Desktop: Full filesystem access via tokio.
//! Web: Stub implementations that return errors or None.

use crate::error::AetherDesktopError;
use std::path::{Path, PathBuf};

/// Read a file's contents as a UTF-8 string.
///
/// Desktop: Reads from the filesystem using tokio.
/// Web: Returns an error (no filesystem access).
#[cfg(feature = "desktop")]
pub async fn read_to_string(path: impl AsRef<Path>) -> Result<String, AetherDesktopError> {
    tokio::fs::read_to_string(path.as_ref())
        .await
        .map_err(|e| AetherDesktopError::FileSearch(e.to_string()))
}

#[cfg(not(feature = "desktop"))]
pub async fn read_to_string(_path: impl AsRef<Path>) -> Result<String, AetherDesktopError> {
    Err(AetherDesktopError::FileSearch(
        "File reading not supported in web mode".to_string(),
    ))
}

/// Returns the platform-specific configuration directory.
///
/// Desktop: Uses `dirs::config_dir()` (e.g., `~/.config` on Linux).
/// Web: Returns `None`.
#[cfg(feature = "desktop")]
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir()
}

#[cfg(not(feature = "desktop"))]
pub fn config_dir() -> Option<PathBuf> {
    None
}

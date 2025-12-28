//! Unified error types for aether-desktop.
//!
//! This module consolidates all error types into a single `AetherDesktopError` enum
//! using the `thiserror` crate to reduce boilerplate.

use thiserror::Error;

/// Unified error type for all aether-desktop errors.
///
/// This enum consolidates the following previously separate error types:
/// - `AgentSpawnError` (agent spawning and configuration)
/// - `ActorError` (agent runtime operations)
/// - `DiffError` (git diff computation)
/// - `FileWatcherError` (file system watching)
/// - `SettingsError` (settings loading/saving)
/// - `SendError` (agent communication)
#[derive(Debug, Error)]
pub enum AetherDesktopError {
    #[error("Failed to parse provider: {0}")]
    ProviderParse(String),

    #[error("Failed to parse MCP config: {0}")]
    McpConfigParse(String),

    #[error("Failed to spawn MCP: {0}")]
    McpSpawn(String),

    #[error("Failed to spawn agent: {0}")]
    AgentSpawn(String),

    // Actor Errors
    #[error("Failed to spawn agent process: {0}")]
    ActorSpawn(String),

    #[error("Failed to initialize agent: {0}")]
    ActorInit(String),

    #[error("Failed to create session: {0}")]
    ActorSession(String),

    // Diff Errors
    #[error("Not a git repository")]
    DiffNotRepository,

    #[error("Diff entry has no file path")]
    DiffMissingPath,

    #[error("Git error: {0}")]
    DiffGit(git2::Error),

    // File Watcher Errors
    #[error("Failed to create file watcher: {0}")]
    FileWatcherCreation(String),

    #[error("Failed to watch path: {0}")]
    FileWatcherPath(String),

    // Settings Errors
    #[error("Settings IO error: {0}")]
    SettingsIo(#[from] std::io::Error),

    #[error("Settings parse error: {0}")]
    SettingsParse(#[from] serde_json::Error),

    // Send Errors
    #[error("Agent not connected")]
    SendNotConnected,

    #[error("Agent channel closed")]
    SendChannelClosed,

    #[error("File search error: {0}")]
    FileSearch(String),
}

impl From<git2::Error> for AetherDesktopError {
    fn from(e: git2::Error) -> Self {
        if e.code() == git2::ErrorCode::NotFound {
            AetherDesktopError::DiffNotRepository
        } else {
            AetherDesktopError::DiffGit(e)
        }
    }
}

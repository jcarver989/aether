//! Error types for coding tools
//!
//! This module provides structured error types for all coding tool operations,
//! replacing the previous `Result<T, String>` pattern with proper `thiserror` enums.

use thiserror::Error;

use super::lsp::error::LspError;

/// Top-level error type for all coding tool operations
#[derive(Debug, Error)]
pub enum CodingError {
    /// File operation errors (read, write, edit)
    #[error(transparent)]
    File(#[from] FileError),

    /// Bash command execution errors
    #[error(transparent)]
    Bash(#[from] BashError),

    /// Grep search errors
    #[error(transparent)]
    Grep(#[from] GrepError),

    /// Find file errors
    #[error(transparent)]
    Find(#[from] FindError),

    /// List files errors
    #[error(transparent)]
    ListFiles(#[from] ListFilesError),

    /// LSP-related errors
    #[error(transparent)]
    Lsp(#[from] LspError),

    /// Tool not configured/available
    #[error("{0}")]
    NotConfigured(String),
}

/// Errors related to file operations (read, write, edit)
#[derive(Debug, Error)]
pub enum FileError {
    /// File does not exist
    #[error("File does not exist: {path}")]
    NotFound { path: String },

    /// Failed to read file
    #[error("Failed to read file {path}: {reason}")]
    ReadFailed { path: String, reason: String },

    /// Failed to write file
    #[error("Failed to write to file {path}: {reason}")]
    WriteFailed { path: String, reason: String },

    /// Failed to create parent directories
    #[error("Failed to create directories for {path}: {reason}")]
    CreateDirFailed { path: String, reason: String },

    /// Invalid offset (must be 1-indexed)
    #[error("Invalid offset for file {path}: offset must be 1-indexed (start from 1)")]
    InvalidOffset { path: String },

    /// String replacement failed (string not found)
    #[error("String replacement failed for file {path}: string '{pattern}' not found")]
    PatternNotFound { path: String, pattern: String },

    /// IO error (wraps std::io::Error)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors related to bash command execution
#[derive(Debug, Error)]
pub enum BashError {
    /// Command is forbidden (e.g., rm without flags)
    #[error("{0}")]
    Forbidden(String),

    /// Timeout exceeds maximum allowed
    #[error("Timeout cannot exceed 600000ms (10 minutes)")]
    TimeoutTooLarge,

    /// Failed to spawn process
    #[error("Failed to execute command '{command}': {reason}")]
    SpawnFailed { command: String, reason: String },

    /// Invalid regex pattern for filtering
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(String),

    /// Failed to join background task
    #[error("Failed to join background task: {0}")]
    JoinFailed(String),

    /// Shell ID not found
    #[error("Shell ID not found: {0}")]
    ShellNotFound(String),

    /// Wait on child process failed
    #[error("Wait failed: {0}")]
    WaitFailed(String),
}

/// Errors related to grep search operations
#[derive(Debug, Error)]
pub enum GrepError {
    /// Invalid glob pattern
    #[error("Invalid glob pattern '{pattern}': {reason}")]
    InvalidGlobPattern { pattern: String, reason: String },

    /// Failed to build glob set
    #[error("Failed to build glob set: {0}")]
    GlobSetBuildFailed(String),

    /// Invalid regex pattern
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(String),

    /// Search error during file processing
    #[error("Search error: {0}")]
    SearchFailed(String),
}

/// Errors related to find file operations
#[derive(Debug, Error)]
pub enum FindError {
    /// Search path does not exist
    #[error("Search path does not exist: {0}")]
    PathNotFound(String),

    /// Invalid glob pattern
    #[error("Invalid glob pattern '{pattern}': {reason}")]
    InvalidGlobPattern { pattern: String, reason: String },

    /// Failed to lock results (mutex poisoned)
    #[error("Failed to lock results")]
    LockFailed,
}

/// Errors related to list files operations
#[derive(Debug, Error)]
pub enum ListFilesError {
    /// Failed to read directory
    #[error("Failed to read directory: {0}")]
    ReadDirFailed(String),

    /// Failed to read directory entry
    #[error("Failed to read entry: {0}")]
    ReadEntryFailed(String),

    /// Failed to read metadata
    #[error("Failed to read metadata: {0}")]
    MetadataFailed(String),
}

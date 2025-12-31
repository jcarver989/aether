//! Error types for voice recording and transcription.

use thiserror::Error;

/// Errors that can occur during voice recording and transcription.
#[derive(Error, Debug)]
pub enum VoiceError {
    #[error("No audio input device found")]
    NoInputDevice,

    #[error("Audio stream error: {0}")]
    StreamError(String),

    #[error("Transcription error: {0}")]
    TranscriptionError(String),

    #[error("Whisper model not found at {path}")]
    ModelNotFound { path: String },

    #[error("Failed to load Whisper model: {0}")]
    ModelLoadError(String),

    #[error("Model download failed: {0}")]
    ModelDownloadError(String),

    #[error("Audio format conversion error: {0}")]
    FormatError(String),

    #[error("Recording error: {0}")]
    RecordingError(String),

    #[error("Permission denied: microphone access not granted")]
    PermissionDenied,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, VoiceError>;

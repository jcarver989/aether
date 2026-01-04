//! Voice recording platform abstraction.
//!
//! Desktop: Full voice recording with streaming transcription.
//! Web: Stub types only (voice not supported).

// =============================================================================
// Desktop: Real voice implementation
// =============================================================================

#[cfg(feature = "desktop")]
pub use aether_voice::{RecordingState, TranscriptionUpdate, record_and_transcribe};

#[cfg(feature = "desktop")]
pub use tokio::sync::oneshot;

// =============================================================================
// Web: Stub types and error-returning functions
// =============================================================================

#[cfg(not(feature = "desktop"))]
pub use crate::fakes::voice::{RecordingState, TranscriptionUpdate};

#[cfg(not(feature = "desktop"))]
pub use futures::channel::oneshot;

/// Start voice recording and transcription.
///
/// Desktop: Real recording with streaming transcription.
/// Web: Returns error (voice not supported).
#[cfg(not(feature = "desktop"))]
pub async fn record_and_transcribe(
    _stop_rx: oneshot::Receiver<()>,
) -> Result<futures::channel::mpsc::Receiver<TranscriptionUpdate>, VoiceNotSupportedError> {
    Err(VoiceNotSupportedError)
}

/// Error returned when voice recording is not supported (web mode).
#[cfg(not(feature = "desktop"))]
#[derive(Debug)]
pub struct VoiceNotSupportedError;

#[cfg(not(feature = "desktop"))]
impl std::fmt::Display for VoiceNotSupportedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Voice recording not supported in web mode")
    }
}

#[cfg(not(feature = "desktop"))]
impl std::error::Error for VoiceNotSupportedError {}

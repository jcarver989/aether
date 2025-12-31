mod audio_recorder;
mod error;
mod state;
mod transcriber;
pub use audio_recorder::AudioRecorder;
pub use error::{Result, VoiceError};
pub use state::RecordingState;
use tokio::{sync::oneshot, task::spawn_blocking};
pub use transcriber::Transcriber;

/// Record audio and transcribe it to text.
pub async fn record_and_transcribe(stop_rx: oneshot::Receiver<()>) -> Result<String> {
    spawn_blocking(move || {
        let audio = AudioRecorder::new()?.record_until_stopped(stop_rx)?;
        if audio.is_empty() {
            return Ok(String::new());
        }

        Transcriber::new()?.transcribe(&audio)
    })
    .await
    .map_err(|e| VoiceError::Internal(format!("Voice task failed: {}", e)))?
}

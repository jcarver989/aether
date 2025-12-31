mod audio_recorder;
mod error;
mod state;
mod streaming;
mod transcriber;

pub use error::{Result, VoiceError};
pub use state::RecordingState;
pub use streaming::TranscriptionUpdate;

use streaming::run_streaming_loop;
use tokio::sync::{mpsc, oneshot};

/// Start streaming transcription.
///
/// Returns a receiver that yields `TranscriptionUpdate`s as the user speaks.
/// Send to `stop_tx` to stop recording - a final transcription will be sent
/// with `is_final: true`.
pub async fn record_and_transcribe(
    stop_rx: oneshot::Receiver<()>,
) -> Result<mpsc::Receiver<TranscriptionUpdate>> {
    let (tx, rx) = mpsc::channel(16);

    tokio::task::spawn_blocking(move || {
        if let Err(e) = run_streaming_loop(stop_rx, tx.clone()) {
            tracing::error!("Streaming transcription error: {}", e);
            let _ = tx.blocking_send(TranscriptionUpdate {
                text: String::new(),
                is_final: true,
            });
        }
    });

    Ok(rx)
}

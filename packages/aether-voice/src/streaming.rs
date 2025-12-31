//! Streaming transcription with periodic updates.

use crate::audio_recorder::AudioRecorder;
use crate::error::Result;
use crate::transcriber::Transcriber;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

/// How often to run transcription (in milliseconds).
const TRANSCRIBE_INTERVAL_MS: u64 = 500;

/// Update from the streaming transcriber.
#[derive(Debug, Clone)]
pub struct TranscriptionUpdate {
    /// Current transcription text.
    pub text: String,
    /// True when recording has stopped and this is the final transcription.
    pub is_final: bool,
}

/// Audio buffer that accumulates all samples during recording.
struct AudioBuffer {
    samples: Vec<f32>,
}

impl AudioBuffer {
    fn new() -> Self {
        // Pre-allocate for ~30 seconds at 16kHz
        Self {
            samples: Vec::with_capacity(16000 * 30),
        }
    }

    fn push(&mut self, new_samples: &[f32]) {
        self.samples.extend_from_slice(new_samples);
    }

    fn as_slice(&self) -> &[f32] {
        &self.samples
    }

    fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// Run the streaming transcription loop.
///
/// This function blocks and should be called from `spawn_blocking`.
/// It reads audio from the recorder, transcribes periodically, and sends updates.
pub fn run_streaming_loop(
    mut stop_rx: oneshot::Receiver<()>,
    tx: mpsc::Sender<TranscriptionUpdate>,
) -> Result<()> {
    let recorder = AudioRecorder::new()?;
    let transcriber = Transcriber::new()?;
    let mut audio_buffer = AudioBuffer::new();

    // Start audio stream that sends samples to a channel
    let (stream, audio_rx) = recorder.start_stream()?;

    // Keep stream alive by holding reference
    let _stream = stream;

    let mut last_transcribe = Instant::now();

    loop {
        // Check for stop signal (non-blocking)
        if stop_rx.try_recv().is_ok() {
            // Final transcription with full buffer
            if !audio_buffer.is_empty() {
                let text = transcriber.transcribe(audio_buffer.as_slice())?;
                let _ = tx.blocking_send(TranscriptionUpdate {
                    text,
                    is_final: true,
                });
            } else {
                let _ = tx.blocking_send(TranscriptionUpdate {
                    text: String::new(),
                    is_final: true,
                });
            }
            break;
        }

        // Drain audio from callback channel into ring buffer
        while let Ok(samples) = audio_rx.try_recv() {
            audio_buffer.push(&samples);
        }

        // Transcribe periodically
        if last_transcribe.elapsed() >= Duration::from_millis(TRANSCRIBE_INTERVAL_MS) {
            if !audio_buffer.is_empty() {
                match transcriber.transcribe(audio_buffer.as_slice()) {
                    Ok(text) => {
                        let _ = tx.blocking_send(TranscriptionUpdate {
                            text,
                            is_final: false,
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Transcription error: {}", e);
                    }
                }
            }
            last_transcribe = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_new() {
        let buffer = AudioBuffer::new();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_audio_buffer_push() {
        let mut buffer = AudioBuffer::new();
        let samples: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        buffer.push(&samples);

        assert_eq!(buffer.as_slice().len(), 1000);
        assert_eq!(buffer.as_slice()[0], 0.0);
        assert_eq!(buffer.as_slice()[999], 999.0);
    }

    #[test]
    fn test_audio_buffer_accumulates() {
        let mut buffer = AudioBuffer::new();

        // Push in chunks - all should be kept
        for i in 0..3 {
            let samples: Vec<f32> = (0..1000).map(|j| (i * 1000 + j) as f32).collect();
            buffer.push(&samples);
        }

        // All 3000 samples should be present
        assert_eq!(buffer.as_slice().len(), 3000);
        assert_eq!(buffer.as_slice()[0], 0.0);
        assert_eq!(buffer.as_slice()[2999], 2999.0);
    }
}

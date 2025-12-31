use crate::error::{Result, VoiceError};
use dirs::config_dir;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use tracing::info;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Model file URL for Whisper base.en model.
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";
const MODEL_FILE: &str = "ggml-base.en.bin";

/// Transcriber using Whisper for speech-to-text.
pub struct Transcriber {
    ctx: WhisperContext,
}

impl Transcriber {
    /// Create a new transcriber, downloading the model if necessary.
    pub fn new() -> Result<Self> {
        let model_path = Self::get_model_path()?;
        if !model_path.exists() {
            Self::download_model(&model_path)?;
        }

        let model_path_str = model_path
            .to_str()
            .ok_or_else(|| VoiceError::Internal("Model path contains invalid UTF-8".to_string()))?;

        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path_str, params)
            .map_err(|e| VoiceError::ModelLoadError(e.to_string()))?;

        Ok(Self { ctx })
    }

    /// Transcribe audio samples to text.
    pub fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        // Less than 0.05 seconds of audio
        if audio.len() < 800 {
            return Ok(String::new());
        }

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| VoiceError::TranscriptionError(e.to_string()))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, audio)
            .map_err(|e| VoiceError::TranscriptionError(e.to_string()))?;

        let result: String = (0..state.full_n_segments())
            .filter_map(|i| {
                state
                    .get_segment(i)
                    .and_then(|s| s.to_str_lossy().ok().map(|s| s.to_string()))
            })
            .collect();

        Ok(result.trim().to_string())
    }

    /// Get the path where the model should be stored.
    fn get_model_path() -> Result<PathBuf> {
        let mut path = config_dir()
            .ok_or_else(|| VoiceError::Internal("Could not find config directory".to_string()))?;

        path.push("aether");
        path.push("models");
        path.push("whisper");
        fs::create_dir_all(&path)?;
        path.push(MODEL_FILE);

        Ok(path)
    }

    /// Download the Whisper model if not present.
    fn download_model(path: &PathBuf) -> Result<()> {
        info!("Downloading Whisper model to: {:?}", path);

        let response = ureq::get(MODEL_URL).call().map_err(|e| {
            VoiceError::ModelDownloadError(format!("Failed to start download: {}", e))
        })?;

        let mut file = File::create(path)?;
        let mut buffer = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut buffer)
            .map_err(|e| VoiceError::ModelDownloadError(format!("Failed to download: {}", e)))?;

        file.write_all(&buffer)?;
        info!("Whisper model downloaded successfully");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcriber_empty_audio() {
        let result = Transcriber::new();
        if let Err(VoiceError::ModelDownloadError(_)) = &result {
            // Expected in environments without network access
            return;
        }

        if let Ok(transcriber) = result {
            let text = transcriber.transcribe(&[]);
            assert!(text.is_ok());
            assert_eq!(text.unwrap(), "");
        }
    }
}

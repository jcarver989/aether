use crate::error::{Result, VoiceError};
use cpal::{
    BuildStreamError, Device, FromSample, PlayStreamError, Sample, SampleFormat, SizedSample,
    Stream, StreamConfig, StreamError, SupportedStreamConfig, default_host,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Whisper requires 16kHz mono audio.
const WHISPER_SAMPLE_RATE: u32 = 16000;

/// Audio recorder that captures microphone input.
pub struct AudioRecorder {
    device: Device,
    config: SupportedStreamConfig,
}

impl AudioRecorder {
    /// Create a new audio recorder using the default input device.
    pub fn new() -> Result<Self> {
        let device = default_host()
            .default_input_device()
            .ok_or(VoiceError::NoInputDevice)?;

        let supported_configs: Vec<_> = device
            .supported_input_configs()
            .map_err(|e| VoiceError::StreamError(format!("Failed to query input configs: {}", e)))?
            .collect();

        if supported_configs.is_empty() {
            return Err(VoiceError::StreamError(
                "No supported input configurations".to_string(),
            ));
        }

        let config = supported_configs
            .iter()
            .find(|c| {
                c.min_sample_rate() <= WHISPER_SAMPLE_RATE
                    && c.max_sample_rate() >= WHISPER_SAMPLE_RATE
            })
            .map(|c| c.with_sample_rate(WHISPER_SAMPLE_RATE))
            .unwrap_or_else(|| supported_configs[0].with_max_sample_rate());

        Ok(Self { device, config })
    }

    /// Record audio until the stop signal is received.
    ///
    /// Returns audio samples normalized to [-1.0, 1.0] at 16kHz mono.
    pub fn record_until_stopped(self, stop_rx: oneshot::Receiver<()>) -> Result<Vec<f32>> {
        let audio_data = Arc::new(Mutex::new(Vec::new()));
        let channels = self.config.channels();
        let sample_rate = self.config.sample_rate();
        let err_handler = |err| {
            eprintln!("An error occurred on the audio stream: {}", err);
        };

        {
            let stream = match self.config.sample_format() {
                SampleFormat::F32 => self.build_stream::<f32>(audio_data.clone(), err_handler)?,
                SampleFormat::I16 => self.build_stream::<i16>(audio_data.clone(), err_handler)?,
                SampleFormat::U16 => self.build_stream::<u16>(audio_data.clone(), err_handler)?,
                sample_format => {
                    return Err(VoiceError::StreamError(format!(
                        "Unsupported sample format: {sample_format}"
                    )));
                }
            };

            stream.play()?;
            let _ = stop_rx.blocking_recv();
        };

        let mono_audio = {
            let raw_audio = audio_data.lock().unwrap();
            if channels > 1 {
                stereo_to_mono(&raw_audio, channels as usize)
            } else {
                raw_audio.clone()
            }
        };

        if sample_rate == WHISPER_SAMPLE_RATE {
            Ok(mono_audio)
        } else {
            Ok(resample(&mono_audio, sample_rate, WHISPER_SAMPLE_RATE))
        }
    }

    fn build_stream<T: Sample + SizedSample>(
        &self,
        audio_data: Arc<Mutex<Vec<f32>>>,
        err_handler: impl FnMut(StreamError) + Send + 'static,
    ) -> Result<Stream>
    where
        f32: FromSample<T>,
    {
        let stream_config: StreamConfig = self.config.clone().into();
        let stream = self.device.build_input_stream(
            &stream_config,
            move |data, _| {
                let mut audio = audio_data.lock().unwrap();
                for &sample in data {
                    audio.push(f32::from_sample(sample));
                }
            },
            err_handler,
            None,
        )?;

        Ok(stream)
    }
}

/// Convert multi-channel audio to mono by averaging channels.
fn stereo_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Resample audio using linear interpolation.
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - idx_floor as f64;
        let sample = samples[idx_floor] * (1.0 - frac as f32) + samples[idx_ceil] * frac as f32;

        output.push(sample);
    }

    output
}

impl From<BuildStreamError> for VoiceError {
    fn from(err: BuildStreamError) -> Self {
        VoiceError::StreamError(err.to_string())
    }
}

impl From<PlayStreamError> for VoiceError {
    fn from(err: PlayStreamError) -> Self {
        VoiceError::StreamError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_creation() {
        let result = AudioRecorder::new();
        if let Err(VoiceError::NoInputDevice) = result {
            // Expected in headless environments
        } else {
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_resample_identity() {
        let samples = vec![0.0, 0.5, 1.0, 0.5, 0.0];
        let resampled = resample(&samples, 16000, 16000);
        assert_eq!(samples, resampled);
    }

    #[test]
    fn test_resample_downsample() {
        let samples: Vec<f32> = (0..48000).map(|i| i as f32 / 48000.0).collect();
        let resampled = resample(&samples, 48000, 16000);
        // Should be roughly 1/3 the length
        assert!((resampled.len() as i32 - 16000).abs() < 10);
    }

    #[test]
    fn test_stereo_to_mono() {
        // Stereo samples: [L, R, L, R, ...]
        let stereo = vec![1.0, 0.0, 0.5, 0.5, 0.0, 1.0];
        let mono = stereo_to_mono(&stereo, 2);
        assert_eq!(mono, vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn test_stereo_to_mono_preserves_mono() {
        let mono_input = vec![0.1, 0.2, 0.3];
        let mono = stereo_to_mono(&mono_input, 1);
        assert_eq!(mono, mono_input);
    }
}

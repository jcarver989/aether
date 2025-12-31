use crate::error::{Result, VoiceError};
use cpal::{
    BuildStreamError, Device, FromSample, PlayStreamError, Sample, SampleFormat, SizedSample,
    Stream, StreamConfig, StreamError, SupportedStreamConfig, default_host,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{Receiver, Sender};

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

    /// Start an audio stream that sends samples to a channel.
    ///
    /// Returns the stream (must be kept alive) and a receiver for audio chunks.
    /// Audio is already converted to 16kHz mono f32 samples.
    pub fn start_stream(self) -> Result<(Stream, Receiver<Vec<f32>>)> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let channels = self.config.channels();
        let sample_rate = self.config.sample_rate();

        let err_handler = |err| {
            eprintln!("An error occurred on the audio stream: {}", err);
        };

        let stream = match self.config.sample_format() {
            SampleFormat::F32 => {
                self.build_streaming::<f32>(tx, channels, sample_rate, err_handler)?
            }
            SampleFormat::I16 => {
                self.build_streaming::<i16>(tx, channels, sample_rate, err_handler)?
            }
            SampleFormat::U16 => {
                self.build_streaming::<u16>(tx, channels, sample_rate, err_handler)?
            }
            sample_format => {
                return Err(VoiceError::StreamError(format!(
                    "Unsupported sample format: {sample_format}"
                )));
            }
        };

        stream.play()?;
        Ok((stream, rx))
    }

    fn build_streaming<T: Sample + SizedSample>(
        &self,
        tx: Sender<Vec<f32>>,
        channels: u16,
        sample_rate: u32,
        err_handler: impl FnMut(StreamError) + Send + 'static,
    ) -> Result<Stream>
    where
        f32: FromSample<T>,
    {
        let stream_config: StreamConfig = self.config.clone().into();
        let needs_resample = sample_rate != WHISPER_SAMPLE_RATE;
        let channels_usize = channels as usize;

        let stream = self.device.build_input_stream(
            &stream_config,
            move |data: &[T], _| {
                // Convert to f32
                let samples: Vec<f32> = data.iter().map(|&s| f32::from_sample(s)).collect();

                // Convert to mono if needed
                let mono = if channels_usize > 1 {
                    stereo_to_mono(&samples, channels_usize)
                } else {
                    samples
                };

                // Resample if needed
                let final_samples = if needs_resample {
                    resample(&mono, sample_rate, WHISPER_SAMPLE_RATE)
                } else {
                    mono
                };

                // Send to channel (ignore errors if receiver dropped)
                let _ = tx.send(final_samples);
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

# Sliding Window Streaming Transcription Plan

## Overview
Add real-time streaming transcription to aether-voice using a sliding window approach with the existing whisper-rs backend. Partial transcriptions stream back as the user speaks, with recent words potentially changing as more context arrives.

## Architecture

```
Microphone (cpal callback)
       ↓
Ring Buffer (last ~3 seconds)
       ↓ (every 500ms)
Transcriber::transcribe()
       ↓
mpsc channel → UI updates
```

## Design Decisions

- **Replace batch API entirely** — streaming becomes the only API
- **No visual distinction** for provisional words — just update text as it arrives

## Files to Modify/Create

| File | Action | Purpose |
|------|--------|---------|
| `aether-voice/src/streaming.rs` | **Create** | Core streaming logic + RingBuffer |
| `aether-voice/src/lib.rs` | Modify | Replace batch API with streaming |
| `aether-voice/src/audio_recorder.rs` | Modify | Add channel-based streaming method |
| `aether-desktop/src/components/prompt_input.rs` | Modify | Consume streaming transcriptions |

## Implementation Details

### 1. New `streaming.rs` Module (~150 lines)

```rust
pub struct StreamingTranscriber {
    transcriber: Transcriber,
    buffer: RingBuffer,          // Keeps last WINDOW_SECONDS of audio
    tx: mpsc::Sender<TranscriptionUpdate>,
}

pub struct TranscriptionUpdate {
    pub text: String,
    pub is_final: bool,          // true when recording stops
    pub error: Option<VoiceError>,
}

pub struct RingBuffer {
    samples: Vec<f32>,
    capacity: usize,             // WINDOW_SECONDS * 16000
}
```

**Key constants:**
- `WINDOW_SECONDS: f32 = 3.0` — How much audio context to keep
- `TRANSCRIBE_INTERVAL_MS: u64 = 500` — How often to transcribe

**Core loop (runs in blocking task):**
```rust
pub fn record_streaming(
    stop_rx: oneshot::Receiver<()>,
    tx: mpsc::Sender<TranscriptionUpdate>,
) -> Result<()> {
    let recorder = AudioRecorder::new()?;
    let transcriber = Transcriber::new()?;
    let mut ring_buffer = RingBuffer::new(WINDOW_SECONDS);

    // cpal callback pushes to a channel instead of Vec
    let (audio_tx, audio_rx) = crossbeam_channel::unbounded();

    // Start audio stream (modified to send to channel)
    let stream = recorder.start_stream(audio_tx)?;

    let mut last_transcribe = Instant::now();

    loop {
        // Check for stop signal (non-blocking)
        if stop_rx.try_recv().is_ok() {
            // Final transcription with full buffer
            let text = transcriber.transcribe(ring_buffer.as_slice())?;
            tx.blocking_send(TranscriptionUpdate { text, is_final: true })?;
            break;
        }

        // Drain audio from callback channel into ring buffer
        while let Ok(samples) = audio_rx.try_recv() {
            ring_buffer.push(&samples);
        }

        // Transcribe periodically
        if last_transcribe.elapsed() >= Duration::from_millis(TRANSCRIBE_INTERVAL_MS) {
            if !ring_buffer.is_empty() {
                let text = transcriber.transcribe(ring_buffer.as_slice())?;
                let _ = tx.blocking_send(TranscriptionUpdate { text, is_final: false });
            }
            last_transcribe = Instant::now();
        }

        thread::sleep(Duration::from_millis(10)); // Prevent busy loop
    }

    Ok(())
}
```

### 2. RingBuffer Implementation (~40 lines)

```rust
impl RingBuffer {
    pub fn new(seconds: f32) -> Self {
        let capacity = (seconds * 16000.0) as usize;
        Self {
            samples: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, new_samples: &[f32]) {
        self.samples.extend_from_slice(new_samples);
        if self.samples.len() > self.capacity {
            let drain_count = self.samples.len() - self.capacity;
            self.samples.drain(0..drain_count);
        }
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.samples
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}
```

### 3. Modify AudioRecorder (~20 lines changed)

Add a new method that sends samples to a channel instead of accumulating:

```rust
impl AudioRecorder {
    // Existing: record_until_stopped() - keep for non-streaming use

    // New: start a stream that sends samples to a channel
    pub fn start_stream(
        &self,
        tx: crossbeam_channel::Sender<Vec<f32>>,
    ) -> Result<cpal::Stream> {
        // Similar to existing, but callback sends to tx instead of pushing to Vec
    }
}
```

### 4. Update lib.rs (~15 lines)

Replace the batch API with streaming:

```rust
mod streaming;

pub use streaming::{record_streaming, TranscriptionUpdate};

// Replace old record_and_transcribe with streaming version
pub async fn record_and_transcribe(
    stop_rx: oneshot::Receiver<()>,
) -> Result<mpsc::Receiver<TranscriptionUpdate>> {
    let (tx, rx) = mpsc::channel(16);

    tokio::task::spawn_blocking(move || {
        if let Err(e) = record_streaming(stop_rx, tx.clone()) {
            let _ = tx.blocking_send(TranscriptionUpdate {
                text: String::new(),
                is_final: true,
                error: Some(e),
            });
        }
    });

    Ok(rx)
}
```

Remove the old batch implementation.

### 5. Update PromptInput Component (~30 lines changed)

```rust
// Instead of waiting for final result:
let mut rx = record_and_transcribe_streaming(stop_rx).await?;

// Spawn task to consume streaming updates
spawn(async move {
    while let Some(update) = rx.recv().await {
        // Update the input field with current transcription
        input_text.set(update.text.clone());

        if update.is_final {
            break;
        }
    }
    recording_state.set(RecordingState::Idle);
});
```

## Dependencies

Add to `aether-voice/Cargo.toml`:
```toml
crossbeam-channel = "0.5"  # For audio callback → main thread communication
```

## Estimated Line Count

| Component | Lines |
|-----------|-------|
| `streaming.rs` (new) | ~150 |
| AudioRecorder changes | ~20 |
| lib.rs changes | ~15 |
| PromptInput changes | ~30 |
| **Total new/changed** | **~215 lines** |

Note: Old batch code in lib.rs gets removed, so net change is smaller.

## Testing Strategy

1. Unit test `RingBuffer` push/capacity behavior
2. Integration test: feed known audio samples, verify transcription updates arrive
3. Manual test: speak into mic, verify text appears progressively

## Future Enhancements (not in scope)

- VAD (Voice Activity Detection) to skip silence
- Word-level confidence/stability markers
- Configurable window size and interval

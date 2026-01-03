//! Unified prompt input component with voice input support.

use super::voice_input::VoiceInput;
use crate::components::layout::{Inline, Space};
use aether_voice::{RecordingState, TranscriptionUpdate, record_and_transcribe};
use dioxus::prelude::*;
use tokio::sync::oneshot;

/// Prompt input with textarea and voice button.
#[component]
pub fn PromptInput(
    value: Signal<String>,
    on_change: EventHandler<String>,
    on_submit: EventHandler<()>,
    placeholder: String,
    disabled: bool,
    rows: Option<&'static str>,
    #[props(default = false)] simple: bool,
) -> Element {
    let voice_state = use_signal(|| RecordingState::Idle);
    let mut stop_tx = use_signal::<Option<oneshot::Sender<()>>>(|| None);
    // Store the text that existed before recording started
    let mut prefix_text = use_signal(String::new);

    let mut handle_voice_click = {
        let mut voice_state = voice_state;
        move |should_start: bool| {
            if should_start {
                if voice_state().can_transition_to(RecordingState::Recording) {
                    voice_state.set(RecordingState::Recording);

                    // Save current text as prefix
                    let current = value();
                    let prefix = if current.is_empty() {
                        String::new()
                    } else {
                        format!("{} ", current)
                    };
                    prefix_text.set(prefix.clone());

                    let (tx, rx) = oneshot::channel();
                    stop_tx.set(Some(tx));

                    spawn(async move {
                        match record_and_transcribe(rx).await {
                            Ok(mut updates_rx) => {
                                // Consume streaming updates
                                while let Some(update) = updates_rx.recv().await {
                                    let TranscriptionUpdate { text, is_final } = update;
                                    // Update input with prefix + transcribed text
                                    let new_value = format!("{}{}", prefix, text);
                                    on_change.call(new_value);

                                    if is_final {
                                        voice_state.set(RecordingState::Idle);
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Voice transcription failed: {}", e);
                                voice_state.set(RecordingState::Error);
                            }
                        }
                    });
                }
            } else {
                // Stop recording immediately when user clicks again
                if let Some(tx) = stop_tx.take() {
                    let _ = tx.send(());
                }
                voice_state.set(RecordingState::Transcribing);
            }
        }
    };

    let rows_attr = rows.unwrap_or("2");

    if simple {
        // Simple version for diff comments (no submit button)
        rsx! {
            div {
                class: "relative",

                textarea {
                    class: "input-field w-full rounded-xl px-4 py-3 resize-none pr-10",
                    value: "{value()}",
                    oninput: move |e: Event<FormData>| {
                        on_change.call(e.value());
                    },
                    placeholder: "{placeholder}",
                    disabled: disabled,
                    rows: rows_attr,
                    autocorrect: "off",
                    spellcheck: "false",
                }

                VoiceInput {
                    state: voice_state,
                    on_toggle_recording: move |_| handle_voice_click(true),
                    on_stop_recording: move |_| handle_voice_click(false),
                }
            }
        }
    } else {
        // Full version with submit button for chat
        rsx! {
            Inline {
                gap: Space::S3,
                align: "items-stretch",

                div {
                    class: "relative flex-1",

                    textarea {
                        class: "input-field w-full rounded-xl px-4 py-3 resize-none pr-10 h-full",
                        value: "{value()}",
                        oninput: move |e: Event<FormData>| {
                            on_change.call(e.value());
                        },
                        onkeydown: move |e: KeyboardEvent| {
                            if e.key() == Key::Enter && !e.modifiers().shift() {
                                e.prevent_default();
                                on_submit.call(());
                            }
                        },
                        placeholder: "{placeholder}",
                        disabled: disabled,
                        rows: rows_attr,
                        autocorrect: "off",
                        spellcheck: "false",
                    }

                    VoiceInput {
                        state: voice_state,
                        on_toggle_recording: move |_| handle_voice_click(true),
                        on_stop_recording: move |_| handle_voice_click(false),
                    }
                }

                button {
                    class: "btn-primary px-6 py-3 rounded-xl font-semibold disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100",
                    onclick: move |_| {
                        on_submit.call(());
                    },
                    disabled: disabled || value().trim().is_empty(),
                    "Send"
                }
            }
        }
    }
}

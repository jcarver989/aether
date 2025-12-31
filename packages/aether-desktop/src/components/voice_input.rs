//! Voice input UI component.

use aether_voice::RecordingState;
use dioxus::prelude::*;

/// Voice input button with recording state indicator.
#[component]
pub fn VoiceInput(
    state: Signal<RecordingState>,
    on_toggle_recording: EventHandler<()>,
    on_stop_recording: EventHandler<()>,
) -> Element {
    let is_recording = matches!(state(), RecordingState::Recording);
    let is_transcribing = matches!(state(), RecordingState::Transcribing);
    let is_error = matches!(state(), RecordingState::Error);

    let button_class = if is_recording {
        "absolute bottom-3 right-3 w-8 h-8 flex items-center justify-center rounded-lg border transition-all cursor-pointer bg-red-500 border-red-500 animate-pulse"
    } else if is_transcribing {
        "absolute bottom-3 right-3 w-8 h-8 flex items-center justify-center rounded-lg border transition-all cursor-pointer bg-accent-primary border-accent-primary"
    } else if is_error {
        "absolute bottom-3 right-3 w-8 h-8 flex items-center justify-center rounded-lg border transition-all cursor-pointer bg-red-500 border-red-500"
    } else {
        "absolute bottom-3 right-3 w-8 h-8 flex items-center justify-center rounded-lg border transition-all cursor-pointer bg-bg-elevated border-border-default hover:bg-bg-tertiary"
    };

    let icon = if is_transcribing {
        // Loading spinner
        rsx! {
            svg {
                class: "animate-spin h-5 w-5",
                xmlns: "http://www.w3.org/2000/svg",
                fill: "none",
                view_box: "0 0 24 24",
                circle {
                    class: "opacity-25",
                    cx: "12",
                    cy: "12",
                    r: "10",
                    stroke: "currentColor",
                    stroke_width: "4"
                }
                path {
                    class: "opacity-75",
                    fill: "currentColor",
                    d: "M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                }
            }
        }
    } else if is_error {
        // Error icon
        rsx! {
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "20",
                height: "20",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                class: "text-white",
                path { d: "M18 6L6 18" }
                path { d: "M6 6l12 12" }
            }
        }
    } else if is_recording {
        // Recording indicator (red square)
        rsx! {
            div { class: "w-3 h-3 bg-red-500 rounded-sm" }
        }
    } else {
        // Microphone icon
        rsx! {
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "20",
                height: "20",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                class: "text-gray-400",
                path { d: "M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" }
                path { d: "M19 10v2a7 7 0 0 1-14 0v-2" }
                line { x1: "12", y1: "19", x2: "12", y2: "23" }
                line { x1: "8", y1: "23", x2: "16", y2: "23" }
            }
        }
    };

    let button_title = if is_recording {
        "Stop recording (click again or press Escape)"
    } else if is_transcribing {
        "Transcribing..."
    } else if is_error {
        "Recording failed - click to retry"
    } else {
        "Start voice input"
    };

    let on_click = move |_| {
        if is_recording || is_error {
            on_stop_recording.call(());
        } else {
            on_toggle_recording.call(());
        }
    };

    rsx! {
        button {
            class: "{button_class}",
            title: "{button_title}",
            onclick: on_click,
            disabled: is_transcribing,
            {icon}
        }
    }
}

//! Recording state machine for voice input.

use crate::error::VoiceError;

/// The current state of the voice recording system.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RecordingState {
    /// Ready to record
    #[default]
    Idle,
    /// Currently recording audio
    Recording,
    /// Transcribing recorded audio
    Transcribing,
    /// An error occurred during recording/transcription
    Error,
}

impl RecordingState {
    /// Check if a transition to the target state is valid.
    pub fn can_transition_to(&self, target: RecordingState) -> bool {
        matches!(
            (*self, target),
            (RecordingState::Idle, RecordingState::Recording)
                | (RecordingState::Recording, RecordingState::Idle)
                | (RecordingState::Recording, RecordingState::Transcribing)
                | (RecordingState::Transcribing, RecordingState::Idle)
                | (_, RecordingState::Error)
                | (RecordingState::Error, RecordingState::Idle)
        )
    }

    /// Attempt to transition to a new state.
    pub fn transition_to(&mut self, target: RecordingState) -> Result<(), VoiceError> {
        if self.can_transition_to(target) {
            *self = target;
            Ok(())
        } else {
            Err(VoiceError::Internal(format!(
                "Invalid state transition: {:?} -> {:?}",
                *self, target
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        let mut state = RecordingState::Idle;

        assert!(state.transition_to(RecordingState::Recording).is_ok());
        assert_eq!(state, RecordingState::Recording);

        assert!(state.transition_to(RecordingState::Transcribing).is_ok());
        assert_eq!(state, RecordingState::Transcribing);

        assert!(state.transition_to(RecordingState::Idle).is_ok());
        assert_eq!(state, RecordingState::Idle);
    }

    #[test]
    fn test_invalid_transitions() {
        let mut state = RecordingState::Recording;

        assert!(state.transition_to(RecordingState::Transcribing).is_ok());
        assert!(state.transition_to(RecordingState::Recording).is_err());
    }

    #[test]
    fn test_error_state_recovery() {
        let mut state = RecordingState::Recording;

        assert!(state.transition_to(RecordingState::Error).is_ok());
        assert_eq!(state, RecordingState::Error);

        assert!(state.transition_to(RecordingState::Idle).is_ok());
        assert_eq!(state, RecordingState::Idle);
    }
}

use agent_client_protocol as acp;
use tokio::sync::mpsc;

/// Commands sent from the main thread to the ACP LocalSet thread.
pub(crate) enum PromptCommand {
    Prompt {
        session_id: acp::SessionId,
        text: String,
    },
    Cancel {
        session_id: acp::SessionId,
    },
}

/// Send-safe handle for issuing prompt commands to the !Send ACP connection.
#[derive(Clone)]
pub struct AcpPromptHandle {
    pub(crate) cmd_tx: mpsc::UnboundedSender<PromptCommand>,
}

impl AcpPromptHandle {
    /// Create a handle that discards all commands. Useful for testing.
    pub fn disconnected() -> Self {
        let (cmd_tx, _rx) = mpsc::unbounded_channel();
        Self { cmd_tx }
    }

    pub fn prompt(&self, session_id: &acp::SessionId, text: &str) {
        let _ = self.cmd_tx.send(PromptCommand::Prompt {
            session_id: session_id.clone(),
            text: text.to_string(),
        });
    }

    pub fn cancel(&self, session_id: &acp::SessionId) {
        let _ = self.cmd_tx.send(PromptCommand::Cancel {
            session_id: session_id.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disconnected_handle_does_not_panic() {
        let handle = AcpPromptHandle::disconnected();
        let session_id = acp::SessionId::new("test");

        // These should silently succeed (receiver is dropped)
        handle.prompt(&session_id, "hello");
        handle.cancel(&session_id);
    }

    #[test]
    fn test_prompt_sends_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };
        let session_id = acp::SessionId::new("sess-1");

        handle.prompt(&session_id, "hello");

        let cmd = rx.try_recv().unwrap();
        match cmd {
            PromptCommand::Prompt { session_id, text } => {
                assert_eq!(session_id.0.as_ref(), "sess-1");
                assert_eq!(text, "hello");
            }
            _ => panic!("Expected Prompt command"),
        }
    }

    #[test]
    fn test_cancel_sends_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };
        let session_id = acp::SessionId::new("sess-1");

        handle.cancel(&session_id);

        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, PromptCommand::Cancel { .. }));
    }
}

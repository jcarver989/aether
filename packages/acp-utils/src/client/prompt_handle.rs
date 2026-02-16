use agent_client_protocol as acp;
use tokio::sync::mpsc;

use super::error::AcpClientError;

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
    /// Create a handle whose sends always succeed but are never read.
    /// Useful for tests that don't care about prompt delivery.
    pub fn noop() -> Self {
        let (cmd_tx, rx) = mpsc::unbounded_channel();
        std::mem::forget(rx);
        Self { cmd_tx }
    }

    pub fn prompt(&self, session_id: &acp::SessionId, text: &str) -> Result<(), AcpClientError> {
        self.cmd_tx
            .send(PromptCommand::Prompt {
                session_id: session_id.clone(),
                text: text.to_string(),
            })
            .map_err(|_| AcpClientError::AgentCrashed("prompt channel closed".into()))
    }

    pub fn cancel(&self, session_id: &acp::SessionId) -> Result<(), AcpClientError> {
        self.cmd_tx
            .send(PromptCommand::Cancel {
                session_id: session_id.clone(),
            })
            .map_err(|_| AcpClientError::AgentCrashed("cancel channel closed".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_handle_succeeds_silently() {
        let handle = AcpPromptHandle::noop();
        let session_id = acp::SessionId::new("test");

        assert!(handle.prompt(&session_id, "hello").is_ok());
        assert!(handle.cancel(&session_id).is_ok());
    }

    #[test]
    fn test_prompt_sends_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };
        let session_id = acp::SessionId::new("sess-1");

        handle.prompt(&session_id, "hello").unwrap();

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

        handle.cancel(&session_id).unwrap();

        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, PromptCommand::Cancel { .. }));
    }
}

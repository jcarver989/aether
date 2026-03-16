use agent_client_protocol as acp;
use std::path::Path;
use tokio::sync::mpsc;

use super::error::AcpClientError;

/// Commands sent from the main thread to the ACP `LocalSet` thread.
#[derive(Debug)]
pub enum PromptCommand {
    Prompt {
        session_id: acp::SessionId,
        text: String,
        content: Option<Vec<acp::ContentBlock>>,
    },
    Cancel {
        session_id: acp::SessionId,
    },
    SetConfigOption {
        session_id: acp::SessionId,
        config_id: String,
        value: String,
    },
    AuthenticateMcpServer {
        session_id: acp::SessionId,
        server_name: String,
    },
    Authenticate {
        #[allow(dead_code)]
        session_id: acp::SessionId,
        method_id: String,
    },
    ListSessions,
    LoadSession {
        session_id: acp::SessionId,
        cwd: std::path::PathBuf,
    },
    NewSession {
        cwd: std::path::PathBuf,
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

    /// Create a handle paired with a receiver for inspecting sent commands.
    /// Useful for tests that need to verify which commands were dispatched.
    pub fn recording() -> (Self, mpsc::UnboundedReceiver<PromptCommand>) {
        let (cmd_tx, rx) = mpsc::unbounded_channel();
        (Self { cmd_tx }, rx)
    }

    pub fn prompt(
        &self,
        session_id: &acp::SessionId,
        text: &str,
        content: Option<Vec<acp::ContentBlock>>,
    ) -> Result<(), AcpClientError> {
        self.send(PromptCommand::Prompt {
            session_id: session_id.clone(),
            text: text.to_string(),
            content,
        })
    }

    pub fn cancel(&self, session_id: &acp::SessionId) -> Result<(), AcpClientError> {
        self.send(PromptCommand::Cancel {
            session_id: session_id.clone(),
        })
    }

    pub fn set_config_option(
        &self,
        session_id: &acp::SessionId,
        config_id: &str,
        value: &str,
    ) -> Result<(), AcpClientError> {
        self.send(PromptCommand::SetConfigOption {
            session_id: session_id.clone(),
            config_id: config_id.to_string(),
            value: value.to_string(),
        })
    }

    pub fn authenticate_mcp_server(
        &self,
        session_id: &acp::SessionId,
        server_name: &str,
    ) -> Result<(), AcpClientError> {
        self.send(PromptCommand::AuthenticateMcpServer {
            session_id: session_id.clone(),
            server_name: server_name.to_string(),
        })
    }

    pub fn authenticate(
        &self,
        session_id: &acp::SessionId,
        method_id: &str,
    ) -> Result<(), AcpClientError> {
        self.send(PromptCommand::Authenticate {
            session_id: session_id.clone(),
            method_id: method_id.to_string(),
        })
    }

    pub fn list_sessions(&self) -> Result<(), AcpClientError> {
        self.send(PromptCommand::ListSessions)
    }

    pub fn load_session(
        &self,
        session_id: &acp::SessionId,
        cwd: &Path,
    ) -> Result<(), AcpClientError> {
        self.send(PromptCommand::LoadSession {
            session_id: session_id.clone(),
            cwd: cwd.to_path_buf(),
        })
    }

    pub fn new_session(&self, cwd: &Path) -> Result<(), AcpClientError> {
        self.send(PromptCommand::NewSession {
            cwd: cwd.to_path_buf(),
        })
    }

    fn send(&self, cmd: PromptCommand) -> Result<(), AcpClientError> {
        self.cmd_tx
            .send(cmd)
            .map_err(|_| AcpClientError::AgentCrashed("command channel closed".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_handle_succeeds_silently() {
        let handle = AcpPromptHandle::noop();
        let session_id = acp::SessionId::new("test");

        assert!(handle.prompt(&session_id, "hello", None).is_ok());
        assert!(handle.cancel(&session_id).is_ok());
    }

    #[test]
    fn test_prompt_sends_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };
        let session_id = acp::SessionId::new("sess-1");

        handle.prompt(&session_id, "hello", None).unwrap();

        let cmd = rx.try_recv().unwrap();
        match cmd {
            PromptCommand::Prompt {
                session_id, text, ..
            } => {
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

    #[test]
    fn test_set_config_option_sends_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };
        let session_id = acp::SessionId::new("sess-1");

        handle
            .set_config_option(&session_id, "model", "gpt-4o")
            .unwrap();

        let cmd = rx.try_recv().unwrap();
        match cmd {
            PromptCommand::SetConfigOption {
                session_id,
                config_id,
                value,
            } => {
                assert_eq!(session_id.0.as_ref(), "sess-1");
                assert_eq!(config_id, "model");
                assert_eq!(value, "gpt-4o");
            }
            _ => panic!("Expected SetConfigOption command"),
        }
    }

    #[test]
    fn test_prompt_with_content_sends_blocks() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };
        let session_id = acp::SessionId::new("sess-1");
        let content = vec![acp::ContentBlock::Text(acp::TextContent::new("attached"))];

        handle
            .prompt(&session_id, "hello", Some(content.clone()))
            .unwrap();

        let cmd = rx.try_recv().unwrap();
        match cmd {
            PromptCommand::Prompt {
                session_id,
                text,
                content: Some(extra),
            } => {
                assert_eq!(session_id.0.as_ref(), "sess-1");
                assert_eq!(text, "hello");
                assert_eq!(extra, content);
            }
            _ => panic!("Expected Prompt command with content"),
        }
    }

    #[test]
    fn test_list_sessions_sends_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };

        handle.list_sessions().unwrap();

        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, PromptCommand::ListSessions));
    }

    #[test]
    fn test_load_session_sends_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };
        let session_id = acp::SessionId::new("sess-restore");
        let cwd = std::path::Path::new("/tmp/project");

        handle.load_session(&session_id, cwd).unwrap();

        let cmd = rx.try_recv().unwrap();
        match cmd {
            PromptCommand::LoadSession { session_id, cwd } => {
                assert_eq!(session_id.0.as_ref(), "sess-restore");
                assert_eq!(cwd, std::path::PathBuf::from("/tmp/project"));
            }
            _ => panic!("Expected LoadSession command"),
        }
    }

    #[test]
    fn test_new_session_sends_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = AcpPromptHandle { cmd_tx: tx };
        let cwd = std::path::Path::new("/tmp/project");

        handle.new_session(cwd).unwrap();

        let cmd = rx.try_recv().unwrap();
        match cmd {
            PromptCommand::NewSession { cwd } => {
                assert_eq!(cwd, std::path::PathBuf::from("/tmp/project"));
            }
            _ => panic!("Expected NewSession command"),
        }
    }
}

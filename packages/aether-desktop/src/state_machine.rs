//! Finite state machine for agent event processing.
//!
//! This module implements pure functions for handling agent events,
//! making the logic unit-testable without Dioxus signals.

use crate::acp_agent::AgentEvent;
use crate::state::{
    AgentSession, AgentStatus, Message, MessageKind, Role, SlashCommand, ToolCallStatus, now_iso,
};
use agent_client_protocol::{AvailableCommand, ToolCallContent};

impl AgentSession {
    /// Apply an event to this agent session, mutating in place.
    pub fn apply_event(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::MessageChunk { text, .. } => self.append_to_streaming_message(text.clone()),
            AgentEvent::MessageComplete { .. } => self.complete_streaming_message(),
            AgentEvent::ToolCallStarted {
                tool_id, tool_call, ..
            } => self.start_tool_call(tool_id.clone(), tool_call.clone()),
            AgentEvent::ToolCallUpdated {
                tool_id, fields, ..
            } => self.update_tool_call(tool_id.clone(), fields.clone()),
            AgentEvent::ToolCallCompleted {
                tool_id, result, ..
            } => self.complete_tool_call(tool_id.clone(), result.clone()),
            AgentEvent::ToolCallFailed { tool_id, error, .. } => {
                self.fail_tool_call(tool_id.clone(), error.clone())
            }
            AgentEvent::StatusChange { status, .. } => self.update_status(status.clone()),
            AgentEvent::AvailableCommandsUpdate { commands, .. } => {
                self.update_available_commands(commands.clone())
            }
            AgentEvent::DiffUpdate { diff_state, .. } => self.update_diff_state(diff_state.clone()),
            AgentEvent::TerminalOutput {
                terminal_id,
                output,
                ..
            } => self.append_terminal_output(terminal_id, output),
            AgentEvent::ContextUsageUpdate {
                usage_ratio,
                tokens_used,
                context_limit,
                ..
            } => self.update_context_usage(*usage_ratio, *tokens_used, *context_limit),
            AgentEvent::Disconnected { .. }
            | AgentEvent::Error { .. }
            | AgentEvent::PermissionRequest { .. } => {
                // These events are handled by the UI consumer directly
            }
        }
    }

    fn append_to_streaming_message(&mut self, text: String) {
        if let Some(msg) = self.messages.last_mut().filter(|m| m.is_streaming)
            && matches!(msg.kind, MessageKind::Text)
        {
            msg.content.push_str(&text);
            return;
        }
        // Create new streaming message
        self.messages.push(Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::Assistant,
            content: text,
            kind: MessageKind::Text,
            timestamp: now_iso(),
            is_streaming: true,
        });
    }

    fn complete_streaming_message(&mut self) {
        if let Some(last_msg) = self.messages.last_mut()
            && last_msg.is_streaming
        {
            last_msg.is_streaming = false;
        }
    }

    fn start_tool_call(&mut self, tool_id: String, tool_call: agent_client_protocol::ToolCall) {
        // Skip if we already have this tool call
        if self.tool_calls.contains_key(&tool_id) {
            return;
        }
        // Skip if message with this ID already exists
        if self.messages.iter().any(|m| m.id == tool_id) {
            return;
        }
        // Mark any streaming message as complete
        self.complete_streaming_message();

        for content in &tool_call.content {
            if let ToolCallContent::Terminal { terminal_id } = content {
                self.terminal_to_tool
                    .insert(terminal_id.to_string(), tool_id.clone());
            }
        }

        let input_content = tool_call
            .raw_input
            .as_ref()
            .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
            .unwrap_or_default();

        self.messages.push(Message {
            id: tool_id.clone(),
            role: Role::Assistant,
            content: input_content,
            kind: MessageKind::ToolCall {
                name: tool_call.title.clone(),
                status: ToolCallStatus::Pending,
                result: None,
            },
            timestamp: now_iso(),
            is_streaming: false,
        });

        self.tool_calls.insert(tool_id, tool_call);
    }

    fn update_tool_call(
        &mut self,
        tool_id: String,
        fields: agent_client_protocol::ToolCallUpdateFields,
    ) {
        if let Some(tc) = self.tool_calls.get_mut(&tool_id) {
            tc.update(fields);
        }
    }

    fn complete_tool_call(&mut self, tool_id: String, result: String) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == tool_id)
            && let MessageKind::ToolCall {
                ref mut status,
                result: ref mut res,
                ..
            } = msg.kind
        {
            *status = ToolCallStatus::Completed;
            *res = Some(result);
        }
    }

    fn fail_tool_call(&mut self, tool_id: String, error: String) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == tool_id)
            && let MessageKind::ToolCall {
                ref mut status,
                result: ref mut res,
                ..
            } = msg.kind
        {
            *status = ToolCallStatus::Failed;
            *res = Some(error);
        }
    }

    fn update_status(&mut self, status: AgentStatus) {
        self.status = status;
    }

    fn update_available_commands(&mut self, commands: Vec<AvailableCommand>) {
        self.available_commands = commands.into_iter().map(SlashCommand::from).collect();
    }

    fn update_diff_state(&mut self, diff_state: crate::state::DiffState) {
        // Preserve the selected file if it still exists in the new diff
        let selected_file = self
            .diff_state
            .selected_file
            .clone()
            .filter(|path| diff_state.files.iter().any(|f| &f.path == path));
        self.diff_state = diff_state;
        self.diff_state.selected_file = selected_file;
    }

    fn update_context_usage(&mut self, usage_ratio: f64, tokens_used: u32, context_limit: u32) {
        self.context_usage = usage_ratio;
        self.tokens_used = tokens_used;
        self.context_limit = context_limit;
    }

    fn append_terminal_output(&mut self, terminal_id: &str, output: &str) {
        let Some(tool_id) = self.terminal_to_tool.get(terminal_id) else {
            return;
        };
        let Some(msg) = self.messages.iter_mut().find(|m| m.id == *tool_id) else {
            return;
        };
        if let MessageKind::ToolCall { result, .. } = &mut msg.kind {
            result.get_or_insert_with(String::new).push_str(output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::ToolCall;
    use std::path::PathBuf;

    fn create_test_session() -> AgentSession {
        // Use a minimal, valid AgentSession for testing
        // We bypass the new() constructor to avoid needing valid SessionId/ToolCallId
        let config = crate::state::AgentConfig::default();
        AgentSession {
            id: "test-id".to_string(),
            acp_session_id: "test-session-id".to_string().into(),
            name: config.name.clone(),
            config,
            status: AgentStatus::Running,
            messages: vec![Message::user_text("Test message")],
            tool_calls: std::collections::HashMap::new(),
            available_commands: Vec::new(),
            cwd: PathBuf::from("/tmp"),
            diff_state: crate::state::DiffState::default(),
            terminal_to_tool: std::collections::HashMap::new(),
            context_usage: 0.0,
            tokens_used: 0,
            context_limit: 0,
        }
    }

    #[test]
    fn test_message_chunk_appends_to_streaming_message() {
        let mut session = create_test_session();
        session.messages.push(Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::Assistant,
            content: "Initial".to_string(),
            kind: MessageKind::Text,
            timestamp: now_iso(),
            is_streaming: true,
        });

        let event = AgentEvent::MessageChunk {
            agent_id: "test-id".to_string(),
            text: " text".to_string(),
        };
        session.apply_event(&event);

        assert_eq!(session.messages.last().unwrap().content, "Initial text");
        assert!(session.messages.last().unwrap().is_streaming);
    }

    #[test]
    fn test_message_chunk_creates_new_message() {
        let mut session = create_test_session();

        let event = AgentEvent::MessageChunk {
            agent_id: "test-id".to_string(),
            text: "New message".to_string(),
        };
        session.apply_event(&event);

        assert_eq!(session.messages.len(), 2); // User message + new message
        assert_eq!(session.messages.last().unwrap().content, "New message");
        assert!(session.messages.last().unwrap().is_streaming);
    }

    #[test]
    fn test_message_complete_stops_streaming() {
        let mut session = create_test_session();
        session.messages.push(Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::Assistant,
            content: "Streaming".to_string(),
            kind: MessageKind::Text,
            timestamp: now_iso(),
            is_streaming: true,
        });

        let event = AgentEvent::MessageComplete {
            agent_id: "test-id".to_string(),
        };
        session.apply_event(&event);

        assert!(!session.messages.last().unwrap().is_streaming);
    }

    #[test]
    fn test_tool_call_started() {
        let mut session = create_test_session();

        let dummy_tool_id = "tool-call-12345".to_string();
        let tool_call = ToolCall {
            id: dummy_tool_id.clone().into(),
            title: "Test Tool".to_string(),
            kind: Default::default(),
            status: agent_client_protocol::ToolCallStatus::Pending,
            content: vec![],
            locations: vec![],
            raw_input: Some(serde_json::json!({"test": "data"})),
            raw_output: None,
            meta: None,
        };

        let event = AgentEvent::ToolCallStarted {
            agent_id: "test-id".to_string(),
            tool_id: "tool-1".to_string(),
            tool_call: tool_call.clone(),
        };
        session.apply_event(&event);

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages.last().unwrap().id, "tool-1");
        assert!(matches!(
            session.messages.last().unwrap().kind,
            MessageKind::ToolCall { .. }
        ));
        assert!(session.tool_calls.contains_key("tool-1"));
    }

    #[test]
    fn test_tool_call_completed() {
        let mut session = create_test_session();
        let tool_id = "tool-1".to_string();

        session.messages.push(Message {
            id: tool_id.clone(),
            role: Role::Assistant,
            content: "Tool call".to_string(),
            kind: MessageKind::ToolCall {
                name: "Test Tool".to_string(),
                status: ToolCallStatus::Pending,
                result: None,
            },
            timestamp: now_iso(),
            is_streaming: false,
        });

        let event = AgentEvent::ToolCallCompleted {
            agent_id: "test-id".to_string(),
            tool_id: tool_id.clone(),
            result: "Success!".to_string(),
        };
        session.apply_event(&event);

        if let MessageKind::ToolCall { status, result, .. } = &session.messages.last().unwrap().kind
        {
            assert_eq!(*status, ToolCallStatus::Completed);
            assert_eq!(result, &Some("Success!".to_string()));
        } else {
            panic!("Expected ToolCall message kind");
        }
    }

    #[test]
    fn test_tool_call_failed() {
        let mut session = create_test_session();
        let tool_id = "tool-1".to_string();

        session.messages.push(Message {
            id: tool_id.clone(),
            role: Role::Assistant,
            content: "Tool call".to_string(),
            kind: MessageKind::ToolCall {
                name: "Test Tool".to_string(),
                status: ToolCallStatus::Pending,
                result: None,
            },
            timestamp: now_iso(),
            is_streaming: false,
        });

        let event = AgentEvent::ToolCallFailed {
            agent_id: "test-id".to_string(),
            tool_id: tool_id.clone(),
            error: "Error!".to_string(),
        };
        session.apply_event(&event);

        if let MessageKind::ToolCall { status, result, .. } = &session.messages.last().unwrap().kind
        {
            assert_eq!(*status, ToolCallStatus::Failed);
            assert_eq!(result, &Some("Error!".to_string()));
        } else {
            panic!("Expected ToolCall message kind");
        }
    }

    #[test]
    fn test_status_change() {
        let mut session = create_test_session();

        let event = AgentEvent::StatusChange {
            agent_id: "test-id".to_string(),
            status: AgentStatus::Error("Test error".to_string()),
        };
        session.apply_event(&event);

        assert!(matches!(session.status, AgentStatus::Error(_)));
    }

    #[test]
    fn test_available_commands_update() {
        let mut session = create_test_session();

        let commands = vec![AvailableCommand {
            name: "test-command".to_string(),
            description: "Test description".to_string(),
            input: None,
            meta: None,
        }];

        let event = AgentEvent::AvailableCommandsUpdate {
            agent_id: "test-id".to_string(),
            commands,
        };
        session.apply_event(&event);

        assert_eq!(session.available_commands.len(), 1);
        assert_eq!(session.available_commands[0].name, "test-command");
    }

    #[test]
    fn test_diff_update_preserves_selected_file() {
        let mut session = create_test_session();
        session.diff_state.selected_file = Some("file1.rs".to_string());

        let mut new_diff = crate::state::DiffState::default();
        new_diff.files = vec![crate::state::FileDiff {
            path: "file1.rs".to_string(),
            old_path: None,
            status: crate::state::FileStatus::Modified,
            hunks: vec![],
        }];

        let event = AgentEvent::DiffUpdate {
            agent_id: "test-id".to_string(),
            diff_state: new_diff,
        };
        session.apply_event(&event);

        assert_eq!(
            session.diff_state.selected_file,
            Some("file1.rs".to_string())
        );
    }

    #[test]
    fn test_diff_update_clears_selected_file_if_missing() {
        let mut session = create_test_session();
        session.diff_state.selected_file = Some("file1.rs".to_string());

        let new_diff = crate::state::DiffState::default();

        let event = AgentEvent::DiffUpdate {
            agent_id: "test-id".to_string(),
            diff_state: new_diff,
        };
        session.apply_event(&event);

        assert!(session.diff_state.selected_file.is_none());
    }

    #[test]
    fn test_mutation_behavior() {
        let mut session = create_test_session();
        assert!(matches!(session.status, AgentStatus::Running));

        let event = AgentEvent::StatusChange {
            agent_id: "test-id".to_string(),
            status: AgentStatus::Idle,
        };
        session.apply_event(&event);

        // Session should be mutated in place
        assert!(matches!(session.status, AgentStatus::Idle));
    }

    #[test]
    fn test_context_usage_update() {
        let mut session = create_test_session();
        assert_eq!(session.context_usage, 0.0);
        assert_eq!(session.tokens_used, 0);
        assert_eq!(session.context_limit, 0);

        let event = AgentEvent::ContextUsageUpdate {
            agent_id: "test-id".to_string(),
            usage_ratio: 0.75,
            tokens_used: 75000,
            context_limit: 100000,
        };
        session.apply_event(&event);

        assert_eq!(session.context_usage, 0.75);
        assert_eq!(session.tokens_used, 75000);
        assert_eq!(session.context_limit, 100000);
    }

    #[test]
    fn test_terminal_output_appends_to_tool_call() {
        let mut session = create_test_session();
        let tool_id = "tool-123".to_string();
        let terminal_id = "term-456".to_string();

        session.messages.push(Message {
            id: tool_id.clone(),
            role: Role::Assistant,
            content: "echo hello".to_string(),
            kind: MessageKind::ToolCall {
                name: "Bash".to_string(),
                status: ToolCallStatus::Pending,
                result: None,
            },
            timestamp: now_iso(),
            is_streaming: false,
        });

        // Manually add the terminal → tool mapping (simulating what start_tool_call does)
        session
            .terminal_to_tool
            .insert(terminal_id.clone(), tool_id.clone());

        // Send terminal output
        let event = AgentEvent::TerminalOutput {
            agent_id: "test-id".to_string(),
            terminal_id: terminal_id.clone(),
            output: "hello\n".to_string(),
            stream: crate::state::TerminalStream::Stdout,
        };
        session.apply_event(&event);

        // Verify output was appended
        if let MessageKind::ToolCall { result, .. } = &session.messages.last().unwrap().kind {
            assert_eq!(result, &Some("hello\n".to_string()));
        } else {
            panic!("Expected ToolCall message kind");
        }
    }

    #[test]
    fn test_terminal_output_accumulates() {
        let mut session = create_test_session();
        let tool_id = "tool-123".to_string();
        let terminal_id = "term-456".to_string();

        // Add a tool call message
        session.messages.push(Message {
            id: tool_id.clone(),
            role: Role::Assistant,
            content: "echo hello; echo world".to_string(),
            kind: MessageKind::ToolCall {
                name: "Bash".to_string(),
                status: ToolCallStatus::Pending,
                result: None,
            },
            timestamp: now_iso(),
            is_streaming: false,
        });

        // Add the mapping
        session
            .terminal_to_tool
            .insert(terminal_id.clone(), tool_id.clone());

        // Send multiple terminal output events
        let event1 = AgentEvent::TerminalOutput {
            agent_id: "test-id".to_string(),
            terminal_id: terminal_id.clone(),
            output: "hello\n".to_string(),
            stream: crate::state::TerminalStream::Stdout,
        };
        session.apply_event(&event1);

        let event2 = AgentEvent::TerminalOutput {
            agent_id: "test-id".to_string(),
            terminal_id: terminal_id.clone(),
            output: "world\n".to_string(),
            stream: crate::state::TerminalStream::Stdout,
        };
        session.apply_event(&event2);

        // Verify both outputs were accumulated
        if let MessageKind::ToolCall { result, .. } = &session.messages.last().unwrap().kind {
            assert_eq!(result, &Some("hello\nworld\n".to_string()));
        } else {
            panic!("Expected ToolCall message kind");
        }
    }

    #[test]
    fn test_terminal_output_ignored_without_mapping() {
        let mut session = create_test_session();
        let tool_id = "tool-123".to_string();

        // Add a tool call message but NO terminal mapping
        session.messages.push(Message {
            id: tool_id.clone(),
            role: Role::Assistant,
            content: "echo hello".to_string(),
            kind: MessageKind::ToolCall {
                name: "Bash".to_string(),
                status: ToolCallStatus::Pending,
                result: None,
            },
            timestamp: now_iso(),
            is_streaming: false,
        });

        // Send terminal output for an unmapped terminal
        let event = AgentEvent::TerminalOutput {
            agent_id: "test-id".to_string(),
            terminal_id: "unknown-terminal".to_string(),
            output: "hello\n".to_string(),
            stream: crate::state::TerminalStream::Stdout,
        };
        session.apply_event(&event);

        // Verify result is still None (output was ignored)
        if let MessageKind::ToolCall { result, .. } = &session.messages.last().unwrap().kind {
            assert_eq!(result, &None);
        } else {
            panic!("Expected ToolCall message kind");
        }
    }
}

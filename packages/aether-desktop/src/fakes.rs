//! Fake implementations for web compilation.
//!
//! These types mimic the real behavior of native-only modules with in-memory
//! implementations and canned responses. Used for e2e testing with webdriver.

use crate::error::AetherDesktopError;
use crate::events::{AgentEvent, AppEvent};
use crate::state::{AgentStatus, ExecutionMode};
use aether_acp_client::transform::AcpEvent;
use agent_client_protocol::{AgentCapabilities, ContentBlock, SessionId, ToolCall};
use agent_events::{AgentMessage, ToolCallRequest, ToolCallResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Fake ACP agent module - provides in-memory agent simulation.
pub mod acp_agent {
    use super::*;
    use dioxus::prelude::spawn;

    /// Fake agent handle that simulates agent behavior in-memory.
    ///
    /// Returns canned responses when prompts are sent.
    pub struct AgentHandle {
        /// Locally-generated UUID for this agent
        pub id: String,
        /// Fake ACP session ID
        pub acp_session_id: SessionId,
        /// Agent capabilities (with reasonable defaults)
        pub agent_capabilities: AgentCapabilities,
        /// Event sender for emitting fake events
        event_tx: futures::channel::mpsc::UnboundedSender<AppEvent>,
    }

    impl AgentHandle {
        /// Spawn a fake agent.
        ///
        /// Immediately returns a handle that can simulate agent behavior.
        pub async fn spawn(
            agent_id: String,
            _cmd_ref: &str,
            _cwd: &Path,
            event_tx: futures::channel::mpsc::UnboundedSender<AppEvent>,
            _execution_mode: ExecutionMode,
        ) -> Result<Self, AetherDesktopError> {
            // Simulate a brief startup delay
            gloo_timers::future::TimeoutFuture::new(100).await;

            Ok(Self {
                id: agent_id.clone(),
                acp_session_id: format!("fake-session-{}", agent_id).into(),
                agent_capabilities: AgentCapabilities::default(),
                event_tx,
            })
        }

        /// Send a prompt to the fake agent.
        ///
        /// Emits canned response events after a brief delay.
        /// If the prompt contains "subagent", also emits sub-agent streaming events.
        pub fn send_prompt(&self, prompt: Vec<ContentBlock>) -> Result<(), AetherDesktopError> {
            let agent_id = self.id.clone();
            let event_tx = self.event_tx.clone();

            // Extract text from prompt for response generation
            let prompt_text: String = prompt
                .iter()
                .filter_map(|block| {
                    if let ContentBlock::Text(text_content) = block {
                        Some(text_content.text.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            // Check if this prompt should trigger sub-agent events
            let should_trigger_subagent = prompt_text.to_lowercase().contains("subagent");

            // Spawn async task to emit canned response
            spawn(async move {
                // Brief delay to simulate processing
                gloo_timers::future::TimeoutFuture::new(200).await;

                // Emit status change to running
                let _ = event_tx.unbounded_send(
                    AgentEvent::StatusChange {
                        agent_id: agent_id.clone(),
                        status: AgentStatus::Running,
                    }
                    .into(),
                );

                // If sub-agent trigger detected, emit sub-agent events
                if should_trigger_subagent {
                    emit_subagent_events(&agent_id, &event_tx).await;
                }

                // Emit canned response in chunks
                let response = generate_canned_response(&prompt_text);
                for chunk in response.chars().collect::<Vec<_>>().chunks(10) {
                    let text: String = chunk.iter().collect();
                    let _ = event_tx.unbounded_send(
                        AgentEvent::Protocol {
                            agent_id: agent_id.clone(),
                            event: AcpEvent::MessageChunk { text },
                        }
                        .into(),
                    );
                    gloo_timers::future::TimeoutFuture::new(50).await;
                }

                // Emit message complete
                let _ = event_tx.unbounded_send(
                    AgentEvent::Protocol {
                        agent_id: agent_id.clone(),
                        event: AcpEvent::MessageComplete,
                    }
                    .into(),
                );

                // Emit status change to idle
                let _ = event_tx.unbounded_send(
                    AgentEvent::StatusChange {
                        agent_id: agent_id.clone(),
                        status: AgentStatus::Idle,
                    }
                    .into(),
                );
            });

            Ok(())
        }

        /// Mark the agent as ready to receive events.
        pub fn mark_ready(&mut self) {
            // No-op for fake - already ready
        }

        /// Terminate the fake agent.
        pub async fn terminate(&self, _timeout_secs: i64) -> Result<(), FakeAgentError> {
            let _ = self.event_tx.unbounded_send(
                AgentEvent::Disconnected {
                    agent_id: self.id.clone(),
                }
                .into(),
            );
            Ok(())
        }
    }

    /// Fake agent error type.
    #[derive(Debug)]
    pub struct FakeAgentError;

    impl std::fmt::Display for FakeAgentError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Fake agent error")
        }
    }

    impl std::error::Error for FakeAgentError {}

    /// Generate a canned response based on the prompt.
    fn generate_canned_response(prompt: &str) -> String {
        let prompt_lower = prompt.to_lowercase();

        if prompt_lower.contains("hello") || prompt_lower.contains("hi") {
            "Hello! I'm a fake agent running in web mode for e2e testing. How can I help you today?"
                .to_string()
        } else if prompt_lower.contains("help") {
            "I'm here to help! In this testing mode, I can respond to basic prompts and simulate agent behavior. Try asking me something!".to_string()
        } else if prompt_lower.contains("test") {
            "Test acknowledged! The e2e testing infrastructure is working correctly. You can interact with the UI and verify component behavior.".to_string()
        } else if prompt_lower.contains("subagent") {
            "I've spawned a sub-agent to help with your request. You can see its progress in the tool call display.".to_string()
        } else {
            format!(
                "I received your message: \"{}\". In web testing mode, I provide canned responses to simulate agent behavior.",
                if prompt.len() > 50 {
                    format!("{}...", &prompt[..50])
                } else {
                    prompt.to_string()
                }
            )
        }
    }

    /// Emit sub-agent streaming events for testing.
    ///
    /// Simulates a sub-agent that explores the codebase and calls tools.
    async fn emit_subagent_events(
        agent_id: &str,
        event_tx: &futures::channel::mpsc::UnboundedSender<AppEvent>,
    ) {
        let tool_id = format!("fake-tool-{}", uuid::Uuid::new_v4());
        let sub_agent_id = format!("fake-subagent-{}", uuid::Uuid::new_v4());
        let agent_name = "fake-subagent";
        let prompt = "Explore the codebase";

        // 1. Emit ToolCallStarted with SpawnSubAgent metadata
        let tool_call = ToolCall {
            id: tool_id.clone().into(),
            title: "spawn_subagent".to_string(),
            kind: Default::default(),
            status: agent_client_protocol::ToolCallStatus::Pending,
            content: vec![],
            locations: vec![],
            raw_input: Some(serde_json::json!({
                "agent_name": agent_name,
                "prompt": prompt
            })),
            raw_output: None,
            meta: None,
        };
        let _ = event_tx.unbounded_send(
            AgentEvent::Protocol {
                agent_id: agent_id.to_string(),
                event: AcpEvent::ToolCallStarted {
                    tool_id: tool_id.clone(),
                    tool_call,
                },
            }
            .into(),
        );

        gloo_timers::future::TimeoutFuture::new(100).await;

        // 2. Emit SubAgentProgress::Text events (streaming chunks)
        for chunk in ["Exploring ", "the ", "codebase ", "structure..."] {
            let _ = event_tx.unbounded_send(
                AgentEvent::SubAgentProgress {
                    agent_id: agent_id.to_string(),
                    parent_tool_id: tool_id.clone(),
                    sub_agent_id: sub_agent_id.clone(),
                    agent_name: agent_name.to_string(),
                    message: AgentMessage::Text {
                        message_id: uuid::Uuid::new_v4().to_string(),
                        chunk: chunk.to_string(),
                        is_complete: false,
                        model_name: "fake-model".to_string(),
                    },
                }
                .into(),
            );
            gloo_timers::future::TimeoutFuture::new(50).await;
        }

        // 3. Emit SubAgentProgress::ToolCall
        let _ = event_tx.unbounded_send(
            AgentEvent::SubAgentProgress {
                agent_id: agent_id.to_string(),
                parent_tool_id: tool_id.clone(),
                sub_agent_id: sub_agent_id.clone(),
                agent_name: agent_name.to_string(),
                message: AgentMessage::ToolCall {
                    request: ToolCallRequest {
                        id: "fake-call-1".to_string(),
                        name: "read_file".to_string(),
                        arguments: r#"{"path": "src/main.rs"}"#.to_string(),
                    },
                    model_name: "fake-model".to_string(),
                },
            }
            .into(),
        );
        gloo_timers::future::TimeoutFuture::new(100).await;

        // 4. Emit SubAgentProgress::ToolResult
        let _ = event_tx.unbounded_send(
            AgentEvent::SubAgentProgress {
                agent_id: agent_id.to_string(),
                parent_tool_id: tool_id.clone(),
                sub_agent_id: sub_agent_id.clone(),
                agent_name: agent_name.to_string(),
                message: AgentMessage::ToolResult {
                    result: ToolCallResult {
                        id: "fake-call-1".to_string(),
                        name: "read_file".to_string(),
                        arguments: r#"{"path": "src/main.rs"}"#.to_string(),
                        result: "File read successfully".to_string(),
                    },
                    model_name: "fake-model".to_string(),
                },
            }
            .into(),
        );
        gloo_timers::future::TimeoutFuture::new(50).await;

        // 5. Emit SubAgentProgress::Done
        let _ = event_tx.unbounded_send(
            AgentEvent::SubAgentProgress {
                agent_id: agent_id.to_string(),
                parent_tool_id: tool_id.clone(),
                sub_agent_id: sub_agent_id.clone(),
                agent_name: agent_name.to_string(),
                message: AgentMessage::Done,
            }
            .into(),
        );

        // 6. Emit ToolCallCompleted with SpawnSubAgent display metadata
        let result_json = serde_json::json!({
            "_meta": {
                "display": {
                    "type": "SpawnSubAgent",
                    "agentName": agent_name,
                    "prompt": prompt,
                    "taskCount": 1,
                    "taskIndex": 1
                }
            },
            "result": "Sub-agent completed exploration"
        });
        let _ = event_tx.unbounded_send(
            AgentEvent::Protocol {
                agent_id: agent_id.to_string(),
                event: AcpEvent::ToolCallCompleted {
                    tool_id: tool_id.clone(),
                    result: serde_json::to_string(&result_json).unwrap_or_default(),
                },
            }
            .into(),
        );
    }
}

/// Fake file search module - provides in-memory file search simulation.
pub mod file_search {
    use super::*;
    use crate::file_types::FileMatch;
    use futures::lock::Mutex;

    /// Global cache of fake file searchers keyed by working directory.
    #[derive(Default)]
    pub struct FileSearcherCache {
        searchers: HashMap<PathBuf, Arc<Mutex<FileSearcher>>>,
    }

    impl FileSearcherCache {
        pub fn new() -> Self {
            Self::default()
        }

        /// Get or create a fake file searcher for the given directory.
        pub fn get_or_create(&mut self, cwd: PathBuf) -> Arc<Mutex<FileSearcher>> {
            self.searchers
                .entry(cwd.clone())
                .or_insert_with(|| Arc::new(Mutex::new(FileSearcher::new(cwd))))
                .clone()
        }

        /// Check if a searcher exists for the given directory.
        pub fn contains(&self, cwd: &Path) -> bool {
            self.searchers.contains_key(cwd)
        }
    }

    /// Fake file searcher with preset file list.
    pub struct FileSearcher {
        /// Root directory being searched
        root: PathBuf,
        /// Preset files for testing
        files: Vec<FileMatch>,
    }

    impl FileSearcher {
        /// Create a new fake file searcher with preset files.
        pub fn new(root: PathBuf) -> Self {
            let files = generate_fake_files(&root);
            Self { root, files }
        }

        /// Index files - no-op for fake, files are preset.
        pub fn index_files(&mut self) -> Result<usize, AetherDesktopError> {
            Ok(self.files.len())
        }

        /// Search for files matching the query.
        ///
        /// Performs simple substring matching on the preset files.
        pub fn search(&mut self, query: &str, limit: usize) -> Vec<FileMatch> {
            let query_lower = query.to_lowercase();

            self.files
                .iter()
                .filter(|f| query.is_empty() || f.path.to_lowercase().contains(&query_lower))
                .take(limit)
                .cloned()
                .collect()
        }

        /// Check if files have been indexed.
        pub fn is_indexed(&self) -> bool {
            true // Always indexed for fake
        }

        /// Get the root directory being searched.
        pub fn root(&self) -> &Path {
            &self.root
        }

        /// Get the total number of indexed files.
        pub fn file_count(&self) -> usize {
            self.files.len()
        }
    }

    /// Generate a preset list of fake files for testing.
    fn generate_fake_files(root: &Path) -> Vec<FileMatch> {
        let fake_paths = [
            ("src/main.rs", 1024),
            ("src/lib.rs", 512),
            ("src/app.rs", 2048),
            ("src/components/mod.rs", 256),
            ("src/components/button.rs", 768),
            ("src/components/input.rs", 640),
            ("src/utils.rs", 384),
            ("Cargo.toml", 512),
            ("README.md", 1024),
            ("tests/integration.rs", 1536),
        ];

        fake_paths
            .iter()
            .map(|(path, size)| FileMatch {
                path: path.to_string(),
                absolute_path: root.join(path),
                size: *size,
            })
            .collect()
    }
}

/// Fake voice module - provides recording state and transcription types.
pub mod voice {
    /// Recording state for voice input.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub enum RecordingState {
        #[default]
        Idle,
        Recording,
        Transcribing,
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
    }

    /// Transcription update from streaming transcription.
    #[derive(Clone, Debug)]
    pub struct TranscriptionUpdate {
        /// The transcribed text so far.
        pub text: String,
        /// Whether this is the final transcription result.
        pub is_final: bool,
    }
}

/// Fake Docker progress for status display.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum DockerProgress {
    #[default]
    CheckingImage,
    PullingImage {
        progress: f32,
    },
    BuildingImage {
        step: u32,
        total: u32,
    },
    CreatingContainer,
    StartingContainer,
    Initializing,
    Ready,
}

impl DockerProgress {
    /// Returns a human-readable description of the current progress state.
    pub fn text(&self) -> &'static str {
        match self {
            DockerProgress::CheckingImage => "Checking image...",
            DockerProgress::PullingImage { .. } => "Pulling image...",
            DockerProgress::BuildingImage { .. } => "Building image...",
            DockerProgress::CreatingContainer => "Creating container...",
            DockerProgress::StartingContainer => "Starting container...",
            DockerProgress::Initializing => "Initializing...",
            DockerProgress::Ready => "Ready",
        }
    }
}

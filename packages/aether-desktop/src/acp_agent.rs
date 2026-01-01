use crate::diff_engine::compute_diff;
use crate::docker_diff::compute_docker_diff;
use crate::docker_watcher::{DockerFileEvent, DockerFilePoller};
use crate::error::AetherDesktopError;
use crate::file_watcher::{FileWatchEvent, FileWatcher};
use crate::state::{AgentStatus, DiffState, ExecutionMode, TerminalStream};
use aether_acp_client::{
    AcpClient, AcpEvent, AgentError, AgentProcess, DockerConfig, DockerProgress, ImageSource,
    OutputStream, ProgressTx, RawAgentEvent, SessionInfo, SpawnConfig, spawn_agent_process,
    start_session,
};
use agent_client_protocol::{
    Agent, AgentCapabilities, AvailableCommand, ClientSideConnection, ContentBlock, PromptRequest,
    RequestPermissionRequest, RequestPermissionResponse, SessionId, ToolCall, ToolCallUpdateFields,
};
use futures::Stream;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use tokio::sync::{mpsc, oneshot};
use tokio::task::spawn_local;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::{StreamExt, StreamMap};
use tracing::{debug, error, info, warn};

/// Result type for agent initialization, containing session info and process handle.
type InitResult = Result<(SessionInfo, Arc<dyn AgentProcess>), AetherDesktopError>;

/// Runtime handle for an agent process.
///
/// Owns the child process, background thread, and command channel.
/// Not stored in Signal - kept in AgentHandles collection.
pub struct AgentHandle {
    /// Locally-generated UUID for this agent - used as primary identifier in UI
    pub id: String,
    /// ACP session ID - used for protocol communication with child process
    pub acp_session_id: SessionId,
    /// Agent capabilities returned from initialization
    pub agent_capabilities: AgentCapabilities,
    /// Background thread running the agent (kept for cleanup)
    #[allow(dead_code)]
    thread: JoinHandle<()>,
    /// Command sender for communicating with the agent
    cmd_tx: mpsc::UnboundedSender<AgentCommand>,
    /// Gate for deferring ACP event forwarding until the UI is ready
    ready_tx: Option<oneshot::Sender<()>>,
    /// Process handle for lifecycle management (termination)
    process_handle: Arc<dyn AgentProcess>,
}

impl AgentHandle {
    /// Spawn an agent on a dedicated OS thread with its own tokio runtime.
    ///
    /// The agent sends all events through the shared `event_tx` channel.
    /// Awaits until the session is established before returning.
    ///
    /// The `agent_id` must be provided by the caller so they can pre-register
    /// the agent in the UI before spawn begins (to receive progress events).
    pub async fn spawn(
        agent_id: String,
        cmd_ref: &str,
        cwd: &Path,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
        execution_mode: ExecutionMode,
    ) -> Result<Self, AetherDesktopError> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (init_tx, init_rx) = oneshot::channel::<InitResult>();
        let (ready_tx, ready_rx) = oneshot::channel();

        let agent_id_for_thread = agent_id.clone();

        let cmd = cmd_ref.to_string();
        let cwd = cwd.to_path_buf();

        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime for agent");

            // Use LocalSet to support spawn_local for non-Send futures
            let local = tokio::task::LocalSet::new();
            local.block_on(
                &rt,
                run_agent(
                    agent_id_for_thread,
                    cmd,
                    cwd,
                    execution_mode,
                    cmd_rx,
                    event_tx,
                    init_tx,
                    ready_rx,
                ),
            );
        });

        // Await initialization (non-blocking)
        let (session_info, process_handle) = init_rx.await.map_err(|_| {
            AetherDesktopError::ActorInit("Agent thread terminated during init".into())
        })??;

        Ok(Self {
            id: agent_id,
            acp_session_id: session_info.session_id,
            agent_capabilities: session_info.agent_capabilities,
            thread,
            cmd_tx,
            ready_tx: Some(ready_tx),
            process_handle,
        })
    }

    /// Send a prompt to the agent.
    ///
    /// The prompt is a vector of ContentBlocks which can include:
    /// - `ContentBlock::Text` for the user's message
    /// - `ContentBlock::Resource` for embedded file contents (if agent supports embedded_context)
    /// - `ContentBlock::ResourceLink` for file references (fallback when embedded_context not supported)
    pub fn send_prompt(&self, prompt: Vec<ContentBlock>) -> Result<(), AetherDesktopError> {
        self.cmd_tx
            .send(AgentCommand::Prompt {
                acp_session_id: self.acp_session_id.clone(),
                prompt,
            })
            .map_err(|_| AetherDesktopError::SendChannelClosed)
    }

    /// Allow the agent loop to begin forwarding ACP events.
    pub fn mark_ready(&mut self) {
        if let Some(tx) = self.ready_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Terminate the agent and clean up resources.
    ///
    /// Attempts graceful shutdown first, then force kills after timeout.
    pub async fn terminate(&self, timeout_secs: i64) -> Result<(), AgentError> {
        self.process_handle.terminate(timeout_secs).await
    }
}

/// UI-ready events emitted by the agent.
///
/// Each variant includes `agent_id` (UUID) for routing to the correct agent in the UI.
/// These are transformed from `RawAgentEvent` by the agent task loop.
#[derive(Debug)]
pub enum AgentEvent {
    /// Append text chunk to the current streaming message
    MessageChunk { agent_id: String, text: String },
    /// Mark current streaming message as complete
    MessageComplete { agent_id: String },
    /// A new tool call started
    ToolCallStarted {
        agent_id: String,
        tool_id: String,
        tool_call: ToolCall,
    },
    /// Tool call fields updated (but not completed/failed)
    ToolCallUpdated {
        agent_id: String,
        tool_id: String,
        fields: ToolCallUpdateFields,
    },
    /// Tool call completed successfully
    ToolCallCompleted {
        agent_id: String,
        tool_id: String,
        result: String,
    },
    /// Tool call failed
    ToolCallFailed {
        agent_id: String,
        tool_id: String,
        error: String,
    },
    /// Agent status changed
    StatusChange {
        agent_id: String,
        status: AgentStatus,
    },
    /// Permission request needs user response
    PermissionRequest {
        #[allow(dead_code)]
        agent_id: String,
        request: RequestPermissionRequest,
        response_tx: oneshot::Sender<RequestPermissionResponse>,
    },
    /// Agent disconnected
    Disconnected { agent_id: String },
    /// Error occurred
    #[allow(dead_code)]
    Error { agent_id: String, error: String },
    /// Available slash commands updated
    AvailableCommandsUpdate {
        agent_id: String,
        commands: Vec<AvailableCommand>,
    },
    /// Git diff state updated
    DiffUpdate {
        agent_id: String,
        diff_state: DiffState,
    },
    /// Terminal output chunk received from a spawned process
    TerminalOutput {
        agent_id: String,
        terminal_id: String,
        output: String,
        stream: TerminalStream,
    },
    /// Context usage updated
    ContextUsageUpdate {
        agent_id: String,
        usage_ratio: f64,
        tokens_used: u32,
        context_limit: u32,
    },
}

impl AgentEvent {
    /// Extract the agent_id from this event.
    pub fn agent_id(&self) -> &str {
        match self {
            AgentEvent::MessageChunk { agent_id, .. } => agent_id,
            AgentEvent::MessageComplete { agent_id } => agent_id,
            AgentEvent::ToolCallStarted { agent_id, .. } => agent_id,
            AgentEvent::ToolCallUpdated { agent_id, .. } => agent_id,
            AgentEvent::ToolCallCompleted { agent_id, .. } => agent_id,
            AgentEvent::ToolCallFailed { agent_id, .. } => agent_id,
            AgentEvent::StatusChange { agent_id, .. } => agent_id,
            AgentEvent::PermissionRequest { agent_id, .. } => agent_id,
            AgentEvent::Disconnected { agent_id } => agent_id,
            AgentEvent::Error { agent_id, .. } => agent_id,
            AgentEvent::AvailableCommandsUpdate { agent_id, .. } => agent_id,
            AgentEvent::DiffUpdate { agent_id, .. } => agent_id,
            AgentEvent::TerminalOutput { agent_id, .. } => agent_id,
            AgentEvent::ContextUsageUpdate { agent_id, .. } => agent_id,
        }
    }
}

/// Commands that can be sent to the ACP actor.
#[derive(Debug)]
pub enum AgentCommand {
    Prompt {
        acp_session_id: SessionId,
        prompt: Vec<ContentBlock>,
    },
}

/// Internal event types for the StreamMap
#[derive(Debug)]
enum LoopEvent {
    Command(AgentCommand),
    RawEvent(Box<RawAgentEvent>),
    PromptComplete(Result<agent_client_protocol::PromptResponse, agent_client_protocol::Error>),
    FileWatchEvent(FileWatchEvent),
}

type EventStream = Pin<Box<dyn Stream<Item = LoopEvent> + Send>>;

/// Main agent loop running on a dedicated thread.
///
/// Handles initialization, sends session info back via init_tx, then runs the event loop.
#[allow(clippy::too_many_arguments)]
async fn run_agent(
    agent_id: String,
    cmd: String,
    cwd: PathBuf,
    execution_mode: ExecutionMode,
    cmd_rx: mpsc::UnboundedReceiver<AgentCommand>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    init_tx: oneshot::Sender<InitResult>,
    ready_rx: oneshot::Receiver<()>,
) {
    // Separate channel for raw events from AcpClient
    let (raw_tx, raw_rx) = mpsc::unbounded_channel::<RawAgentEvent>();

    // Parse command string into parts for spawner
    let cmd_parts: Vec<String> = cmd.split_whitespace().map(String::from).collect();

    // Create appropriate spawner based on execution mode
    let is_docker = execution_mode.is_docker();
    let spawn_config = match &execution_mode {
        ExecutionMode::Local => SpawnConfig::Local,
        ExecutionMode::Docker { dockerfile_path } => {
            // Pass through API keys from host environment
            let mut env = std::collections::HashMap::new();
            if let Ok(key) = std::env::var("ZAI_API_KEY") {
                env.insert("ZAI_API_KEY".to_string(), key);
            }

            SpawnConfig::Docker(DockerConfig {
                image: ImageSource::Dockerfile(dockerfile_path.clone()),
                mounts: vec![],
                env,
                mount_ssh_keys: true,
                working_dir: "/workspace".to_string(),
            })
        }
    };

    let progress_tx: Option<ProgressTx> = if is_docker {
        let (tx, mut rx) = mpsc::unbounded_channel::<DockerProgress>();
        let event_tx_clone = event_tx.clone();
        let agent_id_clone = agent_id.clone();

        spawn_local(async move {
            while let Some(progress) = rx.recv().await {
                let _ = event_tx_clone.send(AgentEvent::StatusChange {
                    agent_id: agent_id_clone.clone(),
                    status: AgentStatus::Starting(progress),
                });
            }
        });

        Some(tx)
    } else {
        None
    };

    let (process_handle, input, output) =
        match spawn_agent_process(spawn_config, &cwd, cmd_parts, progress_tx).await {
            Ok((agent, input, output)) => (agent, input, output),
            Err(e) => {
                let _ = init_tx.send(Err(AetherDesktopError::ActorSpawn(e.to_string())));
                return;
            }
        };

    if is_docker {
        let _ = event_tx.send(AgentEvent::StatusChange {
            agent_id: agent_id.clone(),
            status: AgentStatus::Starting(DockerProgress::Initializing),
        });
    }

    let (conn, io_future) =
        ClientSideConnection::new(AcpClient::new(raw_tx), input, output, |fut| {
            // Spawn local tasks (LocalBoxFuture is not Send)
            tokio::task::spawn_local(fut);
        });

    // Run IO in the background (also local since io_future is not Send)
    tokio::task::spawn_local(async move {
        if let Err(e) = io_future.await {
            error!("ACP IO error: {}", e);
        }
    });

    let session_info = match start_session(&conn, cwd.clone()).await {
        Ok(info) => info,
        Err(e) => {
            let _ = init_tx.send(Err(AetherDesktopError::ActorInit(e.to_string())));
            return;
        }
    };

    let init_result = Ok((
        session_info,
        Arc::clone(&process_handle) as Arc<dyn AgentProcess>,
    ));
    if init_tx.send(init_result).is_err() {
        // Spawner dropped, no point continuing
        return;
    }

    if ready_rx.await.is_err() {
        warn!(agent_id = %agent_id, "Ready signal dropped; stopping agent loop");
        return;
    }

    // Wrap conn in Rc for sharing with spawn_local tasks
    let conn = Rc::new(conn);

    // Set up StreamMap for concurrent event handling
    let mut streams: StreamMap<&str, EventStream> = StreamMap::new();

    // Add command stream
    streams.insert(
        "cmd",
        Box::pin(UnboundedReceiverStream::new(cmd_rx).map(LoopEvent::Command)),
    );

    // Add raw event stream
    streams.insert(
        "raw",
        Box::pin(UnboundedReceiverStream::new(raw_rx).map(|e| LoopEvent::RawEvent(Box::new(e)))),
    );

    // Set up file watcher for the agent's working directory
    // Use different watcher implementation based on execution mode
    // These variables must be declared here to keep them alive for the entire event loop
    let _docker_poller_shutdown: Option<tokio::sync::oneshot::Sender<()>>;
    let _local_file_watcher: Option<FileWatcher>;

    if is_docker {
        // Docker mode: use polling-based watcher with its own channel
        let (docker_watch_tx, docker_watch_rx) = mpsc::unbounded_channel();
        let (poller, shutdown_tx) =
            DockerFilePoller::new(std::sync::Arc::clone(&process_handle), docker_watch_tx);
        poller.start();
        _docker_poller_shutdown = Some(shutdown_tx);
        _local_file_watcher = None;

        // Map Docker file events to the same LoopEvent type
        streams.insert(
            "file_watch",
            Box::pin(
                UnboundedReceiverStream::new(docker_watch_rx).map(|e| match e {
                    DockerFileEvent::Changed => LoopEvent::FileWatchEvent(FileWatchEvent::Changed),
                    DockerFileEvent::Error(msg) => {
                        LoopEvent::FileWatchEvent(FileWatchEvent::Error(msg))
                    }
                }),
            ),
        );
    } else {
        // Local mode: use standard file watcher with its own channel
        _docker_poller_shutdown = None;
        let (file_watch_tx, file_watch_rx) = mpsc::unbounded_channel();
        match FileWatcher::new(cwd.clone(), file_watch_tx) {
            Ok(watcher) => {
                _local_file_watcher = Some(watcher);
                streams.insert(
                    "file_watch",
                    Box::pin(
                        UnboundedReceiverStream::new(file_watch_rx).map(LoopEvent::FileWatchEvent),
                    ),
                );
            }
            Err(e) => {
                warn!(agent_id = %agent_id, "Failed to create file watcher: {}", e);
                _local_file_watcher = None;
            }
        }
    }

    // Send initial diff state
    let initial_diff = if is_docker {
        // Docker mode: compute diff via exec (async), start with empty state
        DiffState {
            is_ephemeral: true,
            ..Default::default()
        }
    } else {
        compute_diff_state(&cwd)
    };
    let _ = event_tx.send(AgentEvent::DiffUpdate {
        agent_id: agent_id.clone(),
        diff_state: initial_diff,
    });

    // Main event loop - polls all streams concurrently
    while let Some((_, event)) = streams.next().await {
        match event {
            LoopEvent::Command(AgentCommand::Prompt {
                acp_session_id: prompt_acp_session_id,
                prompt,
            }) => {
                // Spawn prompt as a stream so it doesn't block other events
                let conn = Rc::clone(&conn);
                let (tx, rx) = mpsc::channel(1);

                tokio::task::spawn_local(async move {
                    let response = conn
                        .prompt(PromptRequest {
                            session_id: prompt_acp_session_id,
                            prompt,
                            meta: None,
                        })
                        .await;
                    let _ = tx.send(response).await;
                });

                streams.insert(
                    "prompt",
                    Box::pin(
                        tokio_stream::wrappers::ReceiverStream::new(rx)
                            .map(LoopEvent::PromptComplete),
                    ),
                );
            }

            LoopEvent::PromptComplete(result) => {
                streams.remove("prompt");
                match result {
                    Ok(response) => {
                        info!(agent_id = %agent_id, "Prompt completed: {:?}", response.stop_reason);
                        let _ = event_tx.send(AgentEvent::MessageComplete {
                            agent_id: agent_id.clone(),
                        });
                        let _ = event_tx.send(AgentEvent::StatusChange {
                            agent_id: agent_id.clone(),
                            status: AgentStatus::Idle,
                        });
                    }
                    Err(e) => {
                        error!(agent_id = %agent_id, "Prompt failed: {}", e);
                        let _ = event_tx.send(AgentEvent::StatusChange {
                            agent_id: agent_id.clone(),
                            status: AgentStatus::Error(e.to_string()),
                        });
                    }
                }
            }

            LoopEvent::RawEvent(raw_event) => {
                let events = transform_raw_event(&agent_id, *raw_event);
                for event in events {
                    let _ = event_tx.send(event);
                }
            }

            LoopEvent::FileWatchEvent(file_event) => match file_event {
                FileWatchEvent::Changed => {
                    debug!(agent_id = %agent_id, "File change detected, recomputing diff");
                    let diff_state = if is_docker {
                        // Docker mode: compute diff via exec
                        match compute_docker_diff(process_handle.as_ref()).await {
                            Ok(files) => {
                                let mut state = DiffState {
                                    files,
                                    is_ephemeral: true,
                                    ..Default::default()
                                };
                                // Select first file by default if any exist
                                if let Some(first) = state.files.first() {
                                    state.selected_file = Some(first.path.clone());
                                }
                                state
                            }
                            Err(e) => {
                                warn!(agent_id = %agent_id, "Docker diff failed: {}", e);
                                DiffState {
                                    is_ephemeral: true,
                                    error: Some(e.to_string()),
                                    ..Default::default()
                                }
                            }
                        }
                    } else {
                        compute_diff_state(&cwd)
                    };
                    let _ = event_tx.send(AgentEvent::DiffUpdate {
                        agent_id: agent_id.clone(),
                        diff_state,
                    });
                }
                FileWatchEvent::Error(err) => {
                    warn!(agent_id = %agent_id, "File watcher error: {}", err);
                }
            },
        }
    }

    let _ = event_tx.send(AgentEvent::Disconnected {
        agent_id: agent_id.clone(),
    });
}

/// Transform a raw event from AcpClient into UI-ready AgentEvent(s).
///
/// Uses the shared transformation from aether_acp_client and adds agent_id
/// for UI routing.
fn transform_raw_event(agent_id: &str, raw_event: RawAgentEvent) -> Vec<AgentEvent> {
    let acp_events = aether_acp_client::transform_raw_event(raw_event);
    acp_events
        .into_iter()
        .map(|event| map_acp_event_to_agent_event(agent_id, event))
        .collect()
}

/// Map a protocol-level AcpEvent to a UI-ready AgentEvent by adding agent_id.
fn map_acp_event_to_agent_event(agent_id: &str, event: AcpEvent) -> AgentEvent {
    match event {
        AcpEvent::MessageChunk { text } => AgentEvent::MessageChunk {
            agent_id: agent_id.to_string(),
            text,
        },
        AcpEvent::MessageComplete => AgentEvent::MessageComplete {
            agent_id: agent_id.to_string(),
        },
        AcpEvent::ToolCallStarted { tool_id, tool_call } => AgentEvent::ToolCallStarted {
            agent_id: agent_id.to_string(),
            tool_id,
            tool_call,
        },
        AcpEvent::ToolCallUpdated { tool_id, fields } => AgentEvent::ToolCallUpdated {
            agent_id: agent_id.to_string(),
            tool_id,
            fields,
        },
        AcpEvent::ToolCallCompleted { tool_id, result } => AgentEvent::ToolCallCompleted {
            agent_id: agent_id.to_string(),
            tool_id,
            result,
        },
        AcpEvent::ToolCallFailed { tool_id, error } => AgentEvent::ToolCallFailed {
            agent_id: agent_id.to_string(),
            tool_id,
            error,
        },
        AcpEvent::PermissionRequest {
            request,
            response_tx,
        } => AgentEvent::PermissionRequest {
            agent_id: agent_id.to_string(),
            request,
            response_tx,
        },
        AcpEvent::AvailableCommandsUpdate { commands } => AgentEvent::AvailableCommandsUpdate {
            agent_id: agent_id.to_string(),
            commands,
        },
        AcpEvent::TerminalOutput {
            terminal_id,
            output,
            stream,
        } => AgentEvent::TerminalOutput {
            agent_id: agent_id.to_string(),
            terminal_id,
            output,
            stream: match stream {
                OutputStream::Stdout => TerminalStream::Stdout,
                OutputStream::Stderr => TerminalStream::Stderr,
            },
        },
        AcpEvent::ContextUsageUpdate {
            usage_ratio,
            tokens_used,
            context_limit,
        } => AgentEvent::ContextUsageUpdate {
            agent_id: agent_id.to_string(),
            usage_ratio,
            tokens_used,
            context_limit,
        },
    }
}

/// Compute diff state for a given working directory.
fn compute_diff_state(cwd: &Path) -> DiffState {
    match compute_diff(cwd) {
        Ok(files) => DiffState {
            files,
            ..Default::default()
        },
        Err(e) => DiffState {
            error: Some(e.to_string()),
            ..Default::default()
        },
    }
}

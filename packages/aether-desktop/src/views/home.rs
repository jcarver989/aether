//! Home view for the desktop app.
//!
//! This is the main view that displays the agent sidebar and chat interface.

use crate::acp_agent::{AgentEvent, AgentHandle};
use crate::components::{AgentView, Card, EmptyState, Inline, NewAgentForm, SettingsEditor, Sidebar, Space, Stack};
use crate::error::AetherDesktopError;
use crate::settings::Settings;
use crate::state::{AgentConfig, AgentHandles, AgentRegistry, AgentSession, AgentStatus};
use crate::{AGENTS, EventChannel, HANDLES};
use aether_acp_client::DockerProgress;
use agent_client_protocol::{ContentBlock, RequestPermissionOutcome, RequestPermissionResponse};
use dioxus::prelude::*;
use std::env::current_dir;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Timeout in seconds for graceful agent termination before force kill.
const TERMINATE_TIMEOUT_SECS: i64 = 10;

/// Represents which view to show in the right pane
#[derive(Clone, PartialEq)]
enum RightPaneView {
    Empty,
    Agent(String), // Agent UUID
    NewAgentForm,
    Settings,
}

#[component]
pub fn Home() -> Element {
    // AGENTS and HANDLES are global signals defined in main.rs
    let event_channel: EventChannel = use_context();
    let event_tx = event_channel.0;

    let mut right_pane: Signal<RightPaneView> = use_signal(|| RightPaneView::Empty);
    let mut error_message: Signal<Option<String>> = use_signal(|| None);

    let settings: Signal<Settings> = use_signal(Settings::load_or_default);
    use_context_provider(|| settings);

    let on_create_agent = move |(config, initial_message): (AgentConfig, String)| {
        let event_tx = event_tx.clone();
        spawn(async move {
            match create_agent(&AGENTS, &HANDLES, event_tx, config, initial_message).await {
                Ok(agent_id) => {
                    right_pane.set(RightPaneView::Agent(agent_id));
                    error_message.set(None);
                }
                Err(e) => {
                    error_message.set(Some(e.to_string()));
                }
            }
        });
    };

    // Derive selected_id for sidebar highlighting
    let selected_id = match &*right_pane.read() {
        RightPaneView::Agent(id) => Some(id.clone()),
        _ => None,
    };

    let on_terminate_agent = move |agent_id: String| {
        let mut right_pane_clone = right_pane;
        spawn(async move {
            if matches!(&*right_pane_clone.read(), RightPaneView::Agent(id) if id == &agent_id) {
                right_pane_clone.set(RightPaneView::Empty);
            }
            terminate_agent(&AGENTS, &HANDLES, agent_id).await;
        });
    };

    rsx! {
        div {
            class: "flex h-screen bg-bg-primary text-white font-sans",

            Sidebar {
                agents: AGENTS.signal(),
                selected_id: selected_id,
                on_new_agent: move |_| right_pane.set(RightPaneView::NewAgentForm),
                on_select_agent: move |id| right_pane.set(RightPaneView::Agent(id)),
                on_settings: move |_| right_pane.set(RightPaneView::Settings),
                on_terminate: on_terminate_agent,
            }

            match &*right_pane.read() {
                RightPaneView::Empty => rsx! { EmptyState {} },
                RightPaneView::Agent(id) => rsx! { AgentView { key: "{id}", agent_id: id.clone() } },
                RightPaneView::NewAgentForm => rsx! {
                    NewAgentForm {
                        on_create: on_create_agent,
                        on_cancel: move |_| right_pane.set(RightPaneView::Empty),
                    }
                },
                RightPaneView::Settings => rsx! {
                    SettingsEditor {
                        on_close: move |_| right_pane.set(RightPaneView::Empty),
                    }
                },
            }

            // Error toast
            if let Some(err) = error_message.read().as_ref() {
                Card {
                    class: "fixed bottom-4 right-4 border-red-500/30 text-white shadow-2xl max-w-md animate-fade-in z-50",
                    p: Space::S4,
                    Stack {
                        gap: Space::S2,
                        Inline {
                            gap: Space::S3,
                            span { class: "font-semibold text-red-400", "Error" }
                            button {
                                class: "ml-auto text-gray-400 hover:text-white transition-colors p-1 rounded hover:bg-white/10",
                                onclick: move |_| error_message.set(None),
                                "✕"
                            }
                        }
                        p { class: "text-sm text-gray-200 leading-relaxed", "{err}" }
                    }
                }
            }
        }
    }
}

/// Single UI consumer that processes events from all agents.
///
/// This is spawned at the App level using GlobalSignals to avoid
/// Dioxus CopyValue scope warnings.
pub async fn run_ui_consumer(
    mut ui_rx: mpsc::UnboundedReceiver<AgentEvent>,
    agents: &GlobalSignal<AgentRegistry>,
    handles: &GlobalSignal<AgentHandles>,
) {
    info!("UI consumer started");

    while let Some(event) = ui_rx.recv().await {
        apply_agent_event(agents, handles, event);
    }

    info!("UI consumer stopped");
}

fn apply_agent_event(
    agents: &GlobalSignal<AgentRegistry>,
    handles: &GlobalSignal<AgentHandles>,
    event: AgentEvent,
) {
    let agent_id = event.agent_id().to_string();

    // Handle events that need side effects (like PermissionRequest)
    if let AgentEvent::PermissionRequest {
        request,
        response_tx,
        ..
    } = event
    {
        // TODO: Show permission dialog in UI
        warn!("Auto-approving permission request: {:?}", request.tool_call);
        let response = RequestPermissionResponse {
            outcome: RequestPermissionOutcome::Selected {
                option_id: request
                    .options
                    .first()
                    .map(|o| o.id.clone())
                    .unwrap_or_else(|| "allow".into()),
            },
            meta: None,
        };
        let _ = response_tx.send(response);
        return;
    }

    if let AgentEvent::Disconnected { agent_id: id } = event {
        if let Some(mut agent_signal) = agents.read().get(&id) {
            let mut agent = agent_signal.write();
            if matches!(agent.status, AgentStatus::Running) {
                agent.status = AgentStatus::Idle;
            }
        }
        agents.write().remove(&id);
        handles.write().remove(&id);
        return;
    }

    if let AgentEvent::Error {
        agent_id: id,
        error,
    } = event
    {
        if let Some(mut agent_signal) = agents.read().get(&id) {
            agent_signal.write().status = AgentStatus::Error(error);
        }
        return;
    }

    let Some(mut agent_signal) = agents.read().get(&agent_id) else {
        warn!("Event for unknown agent: {}", agent_id);
        return;
    };

    agent_signal.write().apply_event(&event);
}

/// Spawn a new agent on a dedicated thread
async fn create_agent(
    agents: &GlobalSignal<AgentRegistry>,
    handles: &GlobalSignal<AgentHandles>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    config: AgentConfig,
    initial_message: String,
) -> Result<String, AetherDesktopError> {
    let cwd = current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let is_docker = config.execution_mode.is_docker();
    let agent_id = uuid::Uuid::new_v4().to_string();

    // Pre-register the agent so it can receive events during spawn
    let initial_status = if is_docker {
        AgentStatus::Starting(DockerProgress::CheckingImage)
    } else {
        AgentStatus::Running
    };
    let session = AgentSession::new(
        agent_id.clone(),
        String::new().into(),
        config.clone(),
        initial_message.clone(),
        cwd.clone(),
        initial_status,
    );
    agents.write().insert(session);

    let spawn_result = AgentHandle::spawn(
        agent_id.clone(),
        &config.command_line,
        &cwd,
        event_tx,
        config.execution_mode.clone(),
    )
    .await;

    let mut handle = match spawn_result {
        Ok(h) => h,
        Err(e) => {
            agents.write().remove(&agent_id);
            return Err(e);
        }
    };

    // Update session with real session_id and final status
    if let Some(mut agent_signal) = agents.read().get(&agent_id) {
        let mut agent = agent_signal.write();
        agent.acp_session_id = handle.acp_session_id.clone();
        agent.status = AgentStatus::Running;
    }

    handle.send_prompt(vec![ContentBlock::from(initial_message)])?;
    handle.mark_ready();
    handles.write().insert(handle);
    info!(agent_id = %agent_id, "Agent spawned on dedicated thread");
    Ok(agent_id)
}

/// Terminate an agent and clean up its resources.
///
/// This removes the agent from both the UI registry and the handles collection,
/// then terminates the underlying process (local or Docker container).
async fn terminate_agent(
    agents: &GlobalSignal<AgentRegistry>,
    handles: &GlobalSignal<AgentHandles>,
    agent_id: String,
) {
    let handle = handles.write().remove(&agent_id);
    agents.write().remove(&agent_id);
    if let Some(handle) = handle
        && let Err(e) = handle.terminate(TERMINATE_TIMEOUT_SECS).await
    {
        warn!(agent_id = %agent_id, "Failed to terminate agent: {}", e);
    }
}

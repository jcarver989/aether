//! Home view for the desktop app.
//!
//! This is the main view that displays the agent sidebar and chat interface.

use crate::components::{
    AgentView, Card, EmptyState, Inline, McpServersPanel, NewAgentForm, SettingsEditor, Sidebar,
    Space, Stack,
};
use crate::error::AetherDesktopError;
use crate::events::{AgentEvent, AppEvent, McpEvent};
#[cfg(feature = "desktop")]
use crate::mcp_oauth::OAUTH_CALLBACK_PORT;
#[cfg(feature = "desktop")]
use crate::mcp_probe::probe_mcp_servers;
use crate::platform::{AgentHandle, DockerProgress, ReceiverExt, mpsc};
use crate::settings::Settings;
use crate::state::{
    AgentConfig, AgentHandles, AgentRegistry, AgentSession, AgentStatus, McpServerStatus,
};
use crate::{AGENTS, EventChannel, HANDLES, MCP_SERVER_STATUSES};

use agent_client_protocol::{ContentBlock, RequestPermissionOutcome, RequestPermissionResponse};
use dioxus::prelude::*;
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
    McpServers,
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
                on_mcp_servers: move |_| {
                    right_pane.set(RightPaneView::McpServers);
                    #[cfg(feature = "desktop")]
                    tokio::spawn(async move {
                        trigger_mcp_probe().await;
                    });
                },
                on_terminate: on_terminate_agent,
            }

            {match &*right_pane.read() {
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
                RightPaneView::McpServers => rsx! {
                    div {
                        class: "flex-1 flex flex-col p-6",
                        div {
                            class: "flex items-center justify-between mb-6",
                            h2 {
                                class: "text-lg font-semibold text-white",
                                "MCP Servers"
                            }
                            button {
                                class: "text-gray-400 hover:text-white transition-colors p-1 rounded hover:bg-white/10",
                                onclick: move |_| right_pane.set(RightPaneView::Empty),
                                "✕"
                            }
                        }
                        McpServersPanel {}
                    }
                },
            }}

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
    mut ui_rx: mpsc::UnboundedReceiver<AppEvent>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    agents: &GlobalSignal<AgentRegistry>,
    handles: &GlobalSignal<AgentHandles>,
) {
    info!("UI consumer started");

    while let Some(event) = ui_rx.recv_next().await {
        match event {
            AppEvent::Mcp(mcp_event) => apply_mcp_event(mcp_event, event_tx.clone()),
            AppEvent::Agent(agent_event) => apply_agent_event(agents, handles, agent_event),
        }
    }

    info!("UI consumer stopped");
}

/// Handle MCP-related events.
///
/// Returns the new status if the global status should be updated.
fn apply_mcp_event(event: McpEvent, event_tx: mpsc::UnboundedSender<AppEvent>) {
    let (server_name, new_status) = match &event {
        McpEvent::StatusChanged {
            server_name,
            status,
        } => {
            info!("MCP server status changed: {} -> {:?}", server_name, status);
            (server_name.clone(), status.clone())
        }
        McpEvent::StartOAuthFlow {
            server_name,
            base_url,
        } => {
            info!("Starting OAuth flow for server: {}", server_name);
            let server_name_clone = server_name.clone();
            let base_url = base_url.clone();
            tokio::spawn(async move {
                run_oauth_flow(server_name_clone, base_url, event_tx).await;
            });
            (server_name.clone(), McpServerStatus::Connecting)
        }
        McpEvent::OAuthFlowCompleted { server_name } => {
            info!("OAuth flow completed for server: {}", server_name);
            (server_name.clone(), McpServerStatus::Connected)
        }
        McpEvent::OAuthFlowFailed { server_name, error } => {
            warn!("OAuth flow failed for server {}: {}", server_name, error);
            (
                server_name.clone(),
                McpServerStatus::Failed {
                    error: error.clone(),
                },
            )
        }
    };

    MCP_SERVER_STATUSES.write().insert(server_name, new_status);
}

/// Run the OAuth flow for an MCP server.
async fn run_oauth_flow(
    server_name: String,
    base_url: String,
    event_tx: mpsc::UnboundedSender<AppEvent>,
) {
    use crate::mcp_oauth::DesktopOAuthHandler;

    info!("Running OAuth flow for {}", server_name);

    let handler = DesktopOAuthHandler::new(OAUTH_CALLBACK_PORT);
    let result_event = match handler.handle_oauth(&server_name, &base_url, &[]).await {
            Ok(_access_token) => {
                info!("OAuth flow succeeded for {}", server_name);
                // Token is automatically persisted via credential store in aether core library
                // The OAuthFlowCompleted event will update the status to Connected
                McpEvent::OAuthFlowCompleted { server_name }
            }
            Err(e) => {
                warn!("OAuth flow failed for {}: {:?}", server_name, e);
                McpEvent::OAuthFlowFailed {
                    server_name,
                    error: e.to_string(),
                }
            }
        };
    let _ = event_tx.send(result_event.into());
}

/// Trigger MCP server probing and update the global status.
#[cfg(feature = "desktop")]
async fn trigger_mcp_probe() {
    let cwd = std::env::current_dir().unwrap_or_else(|e| {
        warn!("Failed to get current directory: {}", e);
        std::path::PathBuf::new()
    });
    info!("Probing MCP servers from {:?}", cwd);

    let results = probe_mcp_servers(&cwd).await;
    info!("Probe complete, updating {} servers", results.len());

    // Update the global status for each server
    let mut statuses = MCP_SERVER_STATUSES.write();
    for (name, status) in results {
        statuses.insert(name, status);
    }
    drop(statuses); // Explicitly drop the write guard
    info!("MCP server statuses updated");
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
        if let Some(agent) = agents.write().get_mut(&id)
            && matches!(agent.status, AgentStatus::Running)
        {
            agent.status = AgentStatus::Idle;
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
        if let Some(agent) = agents.write().get_mut(&id) {
            agent.status = AgentStatus::Error(error);
        }
        return;
    }

    let mut registry = agents.write();
    let Some(agent) = registry.get_mut(&agent_id) else {
        warn!("Event for unknown agent: {}", agent_id);
        return;
    };

    agent.apply_event(&event);
}

/// Spawn a new agent on a dedicated thread
async fn create_agent(
    agents: &GlobalSignal<AgentRegistry>,
    handles: &GlobalSignal<AgentHandles>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    config: AgentConfig,
    initial_message: String,
) -> Result<String, AetherDesktopError> {
    let cwd = config.project_path.clone();
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
    if let Some(agent) = agents.write().get_mut(&agent_id) {
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

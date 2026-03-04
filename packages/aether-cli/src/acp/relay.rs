use acp_utils::notifications::{
    ELICITATION_METHOD, ElicitationParams, ElicitationResponse, McpNotification, McpRequest,
};
use aether_core::events::{AgentMessage, UserMessage};
use aether_core::mcp::run_mcp_task::McpCommand;
use agent_client_protocol::{self as acp, ExtNotification, SessionId};
use llm::parser::ModelProviderParser;
use mcp_utils::client::ElicitationRequest;
use rmcp::model::{CreateElicitationRequestParams, CreateElicitationResult, ElicitationSchema};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{error, info};

use super::mappers::{
    map_agent_message_to_session_notification, map_agent_message_to_stop_reason,
    try_extract_plan_notification, try_into_ext_notification,
};
use super::session::Session;
use acp_utils::server::AcpActorHandle;

pub(crate) enum SessionCommand {
    Prompt {
        text: String,
        switch_model: Option<String>,
        result_tx: oneshot::Sender<Result<acp::StopReason, RelayError>>,
    },
    Cancel,
}

pub(crate) enum RelayError {
    SwitchModelFailed(String),
    SendPromptFailed(String),
    ChannelClosed,
}

impl fmt::Display for RelayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelayError::SwitchModelFailed(e) => write!(f, "switch model failed: {e}"),
            RelayError::SendPromptFailed(e) => write!(f, "send prompt failed: {e}"),
            RelayError::ChannelClosed => write!(f, "agent channel closed"),
        }
    }
}

pub(crate) struct RelayHandle {
    pub cmd_tx: mpsc::Sender<SessionCommand>,
    pub mcp_request_tx: mpsc::Sender<McpRequest>,
    pub join_handle: JoinHandle<()>,
}

pub(crate) fn spawn_relay(
    session: Session,
    actor_handle: AcpActorHandle,
    acp_session_id: SessionId,
) -> RelayHandle {
    let (cmd_tx, cmd_rx) = mpsc::channel(50);
    let (mcp_request_tx, mcp_request_rx) = mpsc::channel(50);
    let join_handle = tokio::spawn(run_session_relay(
        session,
        cmd_rx,
        mcp_request_rx,
        actor_handle,
        acp_session_id,
    ));
    RelayHandle {
        cmd_tx,
        mcp_request_tx,
        join_handle,
    }
}

async fn run_session_relay(
    session: Session,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    mut mcp_request_rx: mpsc::Receiver<McpRequest>,
    actor_handle: AcpActorHandle,
    acp_session_id: SessionId,
) {
    let Session {
        agent_tx,
        mut agent_rx,
        agent_handle: _agent_handle,
        _mcp_handle,
        mcp_tx,
        mut elicitation_rx,
        initial_server_statuses,
    } = session;

    let notification: ExtNotification = McpNotification::ServerStatus {
        servers: initial_server_statuses,
    }
    .into();

    if let Err(e) = actor_handle.send_ext_notification(notification).await {
        error!("Failed to send initial MCP server status: {:?}", e);
    }

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    SessionCommand::Prompt {
                        text,
                        switch_model,
                        result_tx,
                    } => {
                        let mut ctx = PromptContext {
                            agent_tx: &agent_tx,
                            agent_rx: &mut agent_rx,
                            mcp_tx: &mcp_tx,
                            elicitation_rx: &mut elicitation_rx,
                            mcp_request_rx: &mut mcp_request_rx,
                            cmd_rx: &mut cmd_rx,
                            actor_handle: &actor_handle,
                            acp_session_id: &acp_session_id,
                        };
                        let result = handle_prompt(&mut ctx, text, switch_model).await;
                        let _ = result_tx.send(result);
                    }
                    SessionCommand::Cancel => {
                        info!("Cancel received while idle, ignoring");
                    }
                }
            }
            Some(msg) = mcp_request_rx.recv() => {
                match msg {
                    McpRequest::Authenticate { server_name, .. } => {
                        authenticate_mcp_server(&mcp_tx, &actor_handle, &agent_tx, &server_name).await;
                    }
                }
            }
            else => break,
        }
    }
}

struct PromptContext<'a> {
    agent_tx: &'a mpsc::Sender<UserMessage>,
    agent_rx: &'a mut mpsc::Receiver<AgentMessage>,
    mcp_tx: &'a mpsc::Sender<McpCommand>,
    elicitation_rx: &'a mut mpsc::Receiver<ElicitationRequest>,
    mcp_request_rx: &'a mut mpsc::Receiver<McpRequest>,
    cmd_rx: &'a mut mpsc::Receiver<SessionCommand>,
    actor_handle: &'a AcpActorHandle,
    acp_session_id: &'a SessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancelPolicy {
    ForwardToAgent,
    Ignore,
}

async fn handle_prompt(
    ctx: &mut PromptContext<'_>,
    text: String,
    switch_model: Option<String>,
) -> Result<acp::StopReason, RelayError> {
    if let Some(model) = switch_model {
        let parser = ModelProviderParser::default();
        let (provider, _) = parser
            .parse(&model)
            .map_err(|e| RelayError::SwitchModelFailed(format!("{e}")))?;
        ctx.agent_tx
            .send(UserMessage::SwitchModel(provider))
            .await
            .map_err(|e| RelayError::SwitchModelFailed(format!("{e}")))?;
    }

    if is_clear_command(&text) {
        return handle_clear_context(ctx).await;
    }

    let text = expand_slash_command_if_needed(ctx.mcp_tx, text).await;

    ctx.agent_tx
        .send(UserMessage::text(&text))
        .await
        .map_err(|e| RelayError::SendPromptFailed(format!("{e}")))?;

    // The agent sends Cancelled then Done on cancel. Capture stop reason from Cancelled
    // but keep draining until Done to avoid leaving stale messages in the channel.
    let mut early_stop_reason: Option<acp::StopReason> = None;
    run_turn_loop(
        ctx,
        CancelPolicy::ForwardToAgent,
        "Agent channel closed unexpectedly",
        |msg| match msg {
            AgentMessage::Cancelled { .. } => {
                early_stop_reason = Some(map_agent_message_to_stop_reason(msg));
                None
            }
            AgentMessage::Done => Some(
                early_stop_reason
                    .take()
                    .unwrap_or_else(|| map_agent_message_to_stop_reason(msg)),
            ),
            AgentMessage::Error { .. } => Some(map_agent_message_to_stop_reason(msg)),
            _ => None,
        },
    )
    .await
}

async fn handle_clear_context(ctx: &mut PromptContext<'_>) -> Result<acp::StopReason, RelayError> {
    ctx.agent_tx
        .send(UserMessage::ClearContext)
        .await
        .map_err(|e| RelayError::SendPromptFailed(format!("{e}")))?;

    run_turn_loop(
        ctx,
        CancelPolicy::Ignore,
        "Agent channel closed unexpectedly while clearing context",
        handle_clear_context_message,
    )
    .await
}

async fn run_turn_loop<F>(
    ctx: &mut PromptContext<'_>,
    cancel_policy: CancelPolicy,
    channel_closed_log: &'static str,
    mut on_agent_message: F,
) -> Result<acp::StopReason, RelayError>
where
    F: FnMut(&AgentMessage) -> Option<acp::StopReason>,
{
    loop {
        tokio::select! {
            msg = ctx.agent_rx.recv() => {
                if let Some(msg) = msg {
                    forward_notification(ctx.actor_handle, ctx.acp_session_id, &msg).await;
                    if let Some(reason) = on_agent_message(&msg) {
                        info!("Turn completed, stop reason: {:?}", reason);
                        return Ok(reason);
                    }
                } else {
                    error!("{channel_closed_log}");
                    return Err(RelayError::ChannelClosed);
                }
            }
            Some(elicitation) = ctx.elicitation_rx.recv() => {
                handle_elicitation_request(ctx.actor_handle, elicitation).await;
            }
            Some(msg) = ctx.mcp_request_rx.recv() => {
                match msg {
                    McpRequest::Authenticate { server_name, .. } => {
                        authenticate_mcp_server(ctx.mcp_tx, ctx.actor_handle, ctx.agent_tx, &server_name).await;
                    }
                }
            }
            Some(cmd) = ctx.cmd_rx.recv() => {
                handle_in_flight_command(ctx.agent_tx, cmd, cancel_policy).await;
            }
        }
    }
}

fn handle_clear_context_message(msg: &AgentMessage) -> Option<acp::StopReason> {
    match msg {
        AgentMessage::ContextCleared => Some(acp::StopReason::EndTurn),
        AgentMessage::Error { .. } | AgentMessage::Done => {
            Some(map_agent_message_to_stop_reason(msg))
        }
        _ => None,
    }
}

async fn handle_in_flight_command(
    agent_tx: &mpsc::Sender<UserMessage>,
    cmd: SessionCommand,
    cancel_policy: CancelPolicy,
) {
    match cmd {
        SessionCommand::Cancel => match cancel_policy {
            CancelPolicy::ForwardToAgent => {
                info!("Cancel received during prompt processing");
                let _ = agent_tx.send(UserMessage::Cancel).await;
            }
            CancelPolicy::Ignore => {
                info!("Cancel received while context clear is in progress");
            }
        },
        SessionCommand::Prompt { result_tx, .. } => {
            // Can't process a new prompt while one is in-flight
            let _ = result_tx.send(Err(RelayError::SendPromptFailed(
                "prompt already in progress".to_string(),
            )));
        }
    }
}

fn is_clear_command(text: &str) -> bool {
    text.trim() == "/clear"
}

async fn handle_elicitation_request(
    actor_handle: &AcpActorHandle,
    elicitation: ElicitationRequest,
) {
    let ext_params = build_elicitation_params(&elicitation.request);
    let ext_request = build_ext_request(&ext_params);

    let result = actor_handle.ext_method(ext_request).await;
    let mcp_result = match result {
        Ok(ref response) => parse_elicitation_response(response),
        Err(e) => {
            error!("Failed to send elicitation ext_method: {:?}", e);
            CreateElicitationResult {
                action: rmcp::model::ElicitationAction::Cancel,
                content: None,
            }
        }
    };

    if elicitation.response_sender.send(mcp_result).is_err() {
        error!("Failed to send elicitation response: receiver dropped");
    }
}

fn build_elicitation_params(request: &CreateElicitationRequestParams) -> ElicitationParams {
    match request {
        CreateElicitationRequestParams::FormElicitationParams {
            message,
            requested_schema,
            ..
        } => ElicitationParams {
            message: message.clone(),
            schema: requested_schema.clone(),
        },
        CreateElicitationRequestParams::UrlElicitationParams { message, .. } => ElicitationParams {
            message: message.clone(),
            schema: ElicitationSchema::new(BTreeMap::new()),
        },
    }
}

fn build_ext_request(params: &ElicitationParams) -> acp::ExtRequest {
    let raw = serde_json::value::to_raw_value(params).expect("ElicitationParams is serializable");
    acp::ExtRequest::new(ELICITATION_METHOD, Arc::from(raw))
}

fn parse_elicitation_response(response: &acp::ExtResponse) -> CreateElicitationResult {
    let parsed: Result<ElicitationResponse, _> = serde_json::from_str(response.0.get());

    match parsed {
        Ok(r) => CreateElicitationResult {
            action: r.action,
            content: r.content,
        },
        Err(e) => {
            error!("Failed to parse elicitation response: {:?}", e);
            CreateElicitationResult {
                action: rmcp::model::ElicitationAction::Cancel,
                content: None,
            }
        }
    }
}

async fn expand_slash_command_if_needed(mcp_tx: &mpsc::Sender<McpCommand>, text: String) -> String {
    let Some(slash_command_text) = text.strip_prefix('/') else {
        return text;
    };

    let (command_name, args_text) =
        if let Some(space_idx) = slash_command_text.find(char::is_whitespace) {
            let (cmd, args) = slash_command_text.split_at(space_idx);
            (cmd, args.trim())
        } else {
            (slash_command_text, "")
        };

    match expand_slash_command(mcp_tx, command_name, args_text).await {
        Ok(expanded) => {
            info!(
                "Expanded slash command '{}' -> {} chars",
                command_name,
                expanded.len()
            );
            expanded
        }
        Err(e) => {
            error!("Failed to expand slash command '{}': {}", command_name, e);
            text
        }
    }
}

async fn expand_slash_command(
    mcp_tx: &mpsc::Sender<McpCommand>,
    command_name: &str,
    args_text: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let arguments = parse_slash_command_arguments(args_text);

    let (tx_list, rx_list) = oneshot::channel();
    mcp_tx
        .send(McpCommand::ListPrompts { tx: tx_list })
        .await
        .map_err(|e| format!("Failed to send ListPrompts command: {e}"))?;

    let prompts = rx_list
        .await
        .map_err(|e| format!("Failed to receive prompts: {e}"))??;

    let matching_prompt = prompts
        .iter()
        .find(|p| p.name.split("__").last().unwrap_or("") == command_name)
        .ok_or_else(|| format!("Slash command '{command_name}' not found"))?;

    let namespaced_name = matching_prompt.name.clone();

    let (tx_get, rx_get) = oneshot::channel();
    mcp_tx
        .send(McpCommand::GetPrompt {
            name: namespaced_name.clone(),
            arguments,
            tx: tx_get,
        })
        .await
        .map_err(|e| format!("Failed to send GetPrompt command: {e}"))?;

    let prompt_result = rx_get
        .await
        .map_err(|e| format!("Failed to receive prompt: {e}"))??;

    if let Some(message) = prompt_result.messages.first() {
        match &message.content {
            rmcp::model::PromptMessageContent::Text { text } => Ok(text.clone()),
            _ => Err("Prompt message does not contain text content".into()),
        }
    } else {
        Err("Prompt result contains no messages".into())
    }
}

/// Parse slash command arguments into a map with both positional and special variables.
///
/// Creates an argument map with:
/// - "ARGUMENTS": The full argument string
/// - "1", "2", "3", etc.: Individual positional arguments (1-based)
fn parse_slash_command_arguments(
    args_text: &str,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    if args_text.is_empty() {
        None
    } else {
        let mut arg_map = serde_json::Map::new();

        arg_map.insert(
            "ARGUMENTS".to_string(),
            serde_json::Value::String(args_text.to_string()),
        );

        for (i, arg) in args_text.split_whitespace().enumerate() {
            arg_map.insert(
                (i + 1).to_string(),
                serde_json::Value::String(arg.to_string()),
            );
        }

        Some(arg_map)
    }
}

async fn authenticate_mcp_server(
    mcp_tx: &mpsc::Sender<McpCommand>,
    actor_handle: &AcpActorHandle,
    agent_tx: &mpsc::Sender<UserMessage>,
    name: &str,
) {
    let (tx, rx) = oneshot::channel();
    if let Err(e) = mcp_tx
        .send(McpCommand::AuthenticateServer {
            name: name.to_string(),
            tx,
        })
        .await
    {
        error!("MCP server authentication failed: Failed to send AuthenticateServer command: {e}");
        return;
    }

    let result = match rx.await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            error!("MCP server authentication failed: {e}");
            return;
        }
        Err(e) => {
            error!("MCP server authentication failed: Failed to receive auth result: {e}");
            return;
        }
    };

    let (statuses, tool_definitions) = result;
    let notification: ExtNotification = McpNotification::ServerStatus { servers: statuses }.into();
    if let Err(e) = actor_handle.send_ext_notification(notification).await {
        error!("Failed to send updated MCP server status: {:?}", e);
    }
    if let Err(e) = agent_tx
        .send(UserMessage::UpdateTools(tool_definitions))
        .await
    {
        error!("Failed to send updated tools to agent: {:?}", e);
    }
}

async fn forward_notification(
    actor_handle: &AcpActorHandle,
    acp_session_id: &SessionId,
    msg: &AgentMessage,
) {
    if let Some(notification) =
        map_agent_message_to_session_notification(acp_session_id.clone(), msg)
    {
        if let Err(e) = actor_handle.send_session_notification(notification).await {
            error!("Failed to send session notification: {:?}", e);
        }
    } else if let Some(ext_notification) = try_into_ext_notification(msg)
        && let Err(e) = actor_handle.send_ext_notification(ext_notification).await
    {
        error!("Failed to send ext notification: {:?}", e);
    }

    if let AgentMessage::ToolResult { result_meta, .. } = msg
        && let Some(plan_notif) =
            try_extract_plan_notification(acp_session_id.clone(), result_meta.as_ref())
            && let Err(e) = actor_handle.send_session_notification(plan_notif).await {
                error!("Failed to send plan notification: {:?}", e);
            }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_argument_parsing() {
        let arg_map =
            parse_slash_command_arguments("do a thing that has spaces").expect("Expected Some");
        let expected = serde_json::Map::from_iter([
            (
                "ARGUMENTS".to_string(),
                serde_json::Value::String("do a thing that has spaces".to_string()),
            ),
            ("1".to_string(), serde_json::Value::String("do".to_string())),
            ("2".to_string(), serde_json::Value::String("a".to_string())),
            (
                "3".to_string(),
                serde_json::Value::String("thing".to_string()),
            ),
            (
                "4".to_string(),
                serde_json::Value::String("that".to_string()),
            ),
            (
                "5".to_string(),
                serde_json::Value::String("has".to_string()),
            ),
            (
                "6".to_string(),
                serde_json::Value::String("spaces".to_string()),
            ),
        ]);
        assert_eq!(arg_map, expected);
    }

    #[test]
    fn test_empty_arguments_returns_none() {
        assert!(parse_slash_command_arguments("").is_none());
    }

    #[test]
    fn clear_command_matches_exactly() {
        assert!(is_clear_command("/clear"));
        assert!(is_clear_command("  /clear  "));
        assert!(is_clear_command("\n/clear\t"));
    }

    #[test]
    fn clear_command_rejects_extra_tokens() {
        assert!(!is_clear_command("/clear now"));
        assert!(!is_clear_command("/clear/now"));
        assert!(!is_clear_command("/clear-now"));
    }

    #[test]
    fn clear_context_message_completes_turn() {
        let reason = handle_clear_context_message(&AgentMessage::ContextCleared);
        assert_eq!(reason, Some(acp::StopReason::EndTurn));
    }

    #[tokio::test]
    async fn in_flight_cancel_is_forwarded_for_prompt_turns() {
        let (agent_tx, mut agent_rx) = mpsc::channel(1);
        handle_in_flight_command(
            &agent_tx,
            SessionCommand::Cancel,
            CancelPolicy::ForwardToAgent,
        )
        .await;

        let msg = tokio::time::timeout(std::time::Duration::from_millis(200), agent_rx.recv())
            .await
            .expect("cancel should be forwarded")
            .expect("agent channel should stay open");
        assert!(matches!(msg, UserMessage::Cancel));
    }

    #[tokio::test]
    async fn in_flight_cancel_is_ignored_for_clear_turns() {
        let (agent_tx, mut agent_rx) = mpsc::channel(1);
        handle_in_flight_command(&agent_tx, SessionCommand::Cancel, CancelPolicy::Ignore).await;

        assert!(matches!(
            agent_rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn in_flight_prompt_is_rejected_while_turn_in_progress() {
        let (agent_tx, _agent_rx) = mpsc::channel(1);
        let (result_tx, result_rx) = oneshot::channel();

        handle_in_flight_command(
            &agent_tx,
            SessionCommand::Prompt {
                text: "second prompt".to_string(),
                switch_model: None,
                result_tx,
            },
            CancelPolicy::Ignore,
        )
        .await;

        match result_rx
            .await
            .expect("result channel should receive response")
        {
            Ok(reason) => panic!("expected rejection, got stop reason: {reason:?}"),
            Err(RelayError::SendPromptFailed(message)) => {
                assert_eq!(message, "prompt already in progress");
            }
            Err(other) => panic!("expected SendPromptFailed, got {other}"),
        }
    }

    #[test]
    fn test_build_elicitation_params_from_form() {
        let elicitation = CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "Pick a color".to_string(),
            requested_schema: ElicitationSchema::builder()
                .required_bool("approved")
                .build()
                .unwrap(),
        };

        let params = build_elicitation_params(&elicitation);
        assert_eq!(params.message, "Pick a color");
        assert_eq!(params.schema.properties.len(), 1);
        assert!(params.schema.properties.contains_key("approved"));
    }

    #[test]
    fn test_parse_elicitation_response_accept() {
        let response_json = serde_json::json!({
            "action": "accept",
            "content": { "color": "red" }
        });
        let raw = serde_json::value::to_raw_value(&response_json).unwrap();
        let ext_response = acp::ExtResponse::new(Arc::from(raw));

        let result = parse_elicitation_response(&ext_response);
        assert_eq!(result.action, rmcp::model::ElicitationAction::Accept);
        assert_eq!(result.content, Some(serde_json::json!({ "color": "red" })));
    }

    #[test]
    fn test_parse_elicitation_response_decline() {
        let response_json = serde_json::json!({
            "action": "decline",
            "content": null
        });
        let raw = serde_json::value::to_raw_value(&response_json).unwrap();
        let ext_response = acp::ExtResponse::new(Arc::from(raw));

        let result = parse_elicitation_response(&ext_response);
        assert_eq!(result.action, rmcp::model::ElicitationAction::Decline);
        assert!(result.content.is_none());
    }

    #[test]
    fn test_parse_elicitation_response_invalid_json() {
        let raw: Arc<serde_json::value::RawValue> =
            serde_json::from_str("\"not_an_object\"").unwrap();
        let ext_response = acp::ExtResponse::new(raw);

        let result = parse_elicitation_response(&ext_response);
        assert_eq!(result.action, rmcp::model::ElicitationAction::Cancel);
        assert!(result.content.is_none());
    }
}

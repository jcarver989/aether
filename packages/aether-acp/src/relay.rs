use aether::events::{AgentMessage, UserMessage};
use aether::mcp::run_mcp_task::McpCommand;
use agent_client_protocol::{self as acp, SessionId};
use llm::parser::ModelProviderParser;
use std::fmt;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::mappers::{
    map_agent_message_to_session_notification, map_agent_message_to_stop_reason,
    try_into_ext_notification,
};
use crate::session::Session;
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

pub(crate) fn spawn_relay(
    session: Session,
    actor_handle: AcpActorHandle,
    acp_session_id: SessionId,
) -> (mpsc::Sender<SessionCommand>, JoinHandle<()>) {
    let (cmd_tx, cmd_rx) = mpsc::channel(8);
    let handle = tokio::spawn(run_session_relay(session, cmd_rx, actor_handle, acp_session_id));
    (cmd_tx, handle)
}

async fn run_session_relay(
    session: Session,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    actor_handle: AcpActorHandle,
    acp_session_id: SessionId,
) {
    let Session {
        id: _,
        agent_tx,
        mut agent_rx,
        agent_handle: _agent_handle,
        _mcp_handle,
        mcp_tx,
    } = session;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SessionCommand::Prompt {
                text,
                switch_model,
                result_tx,
            } => {
                let result = handle_prompt(
                    &agent_tx,
                    &mut agent_rx,
                    &mcp_tx,
                    &mut cmd_rx,
                    &actor_handle,
                    &acp_session_id,
                    text,
                    switch_model,
                )
                .await;
                let _ = result_tx.send(result);
            }
            SessionCommand::Cancel => {
                // No-op when idle — session isn't processing a prompt
                info!("Cancel received while idle, ignoring");
            }
        }
    }
}

async fn handle_prompt(
    agent_tx: &mpsc::Sender<UserMessage>,
    agent_rx: &mut mpsc::Receiver<AgentMessage>,
    mcp_tx: &mpsc::Sender<McpCommand>,
    cmd_rx: &mut mpsc::Receiver<SessionCommand>,
    actor_handle: &AcpActorHandle,
    acp_session_id: &SessionId,
    text: String,
    switch_model: Option<String>,
) -> Result<acp::StopReason, RelayError> {
    if let Some(model) = switch_model {
        let parser = ModelProviderParser::default();
        let (provider, _) = parser
            .parse(&model)
            .map_err(|e| RelayError::SwitchModelFailed(format!("{e}")))?;
        agent_tx
            .send(UserMessage::SwitchModel(provider))
            .await
            .map_err(|e| RelayError::SwitchModelFailed(format!("{e}")))?;
    }

    let text = expand_slash_command_if_needed(mcp_tx, text).await;

    agent_tx
        .send(UserMessage::text(&text))
        .await
        .map_err(|e| RelayError::SendPromptFailed(format!("{e}")))?;

    // The agent sends Cancelled then Done on cancel. We capture the stop reason from Cancelled
    // but keep draining until Done to avoid leaving stale messages in the channel.
    // Error is immediately terminal because middleware blocks send Error without a trailing Done.
    let mut early_stop_reason: Option<acp::StopReason> = None;

    loop {
        tokio::select! {
            msg = agent_rx.recv() => {
                match msg {
                    Some(msg) => {
                        forward_notification(actor_handle, acp_session_id, &msg).await;

                        match &msg {
                            AgentMessage::Cancelled { .. } => {
                                early_stop_reason = Some(map_agent_message_to_stop_reason(&msg));
                            }
                            AgentMessage::Done => {
                                let reason = early_stop_reason
                                    .unwrap_or_else(|| map_agent_message_to_stop_reason(&msg));
                                info!("Done received, stop reason: {:?}", reason);
                                return Ok(reason);
                            }
                            AgentMessage::Error { .. } => {
                                let reason = map_agent_message_to_stop_reason(&msg);
                                info!("Error received, stop reason: {:?}", reason);
                                return Ok(reason);
                            }
                            _ => {}
                        }
                    }
                    None => {
                        error!("Agent channel closed unexpectedly");
                        return Err(RelayError::ChannelClosed);
                    }
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    SessionCommand::Cancel => {
                        info!("Cancel received during prompt processing");
                        let _ = agent_tx.send(UserMessage::Cancel).await;
                    }
                    SessionCommand::Prompt { result_tx, .. } => {
                        // Can't process a new prompt while one is in-flight
                        let _ = result_tx.send(Err(RelayError::SendPromptFailed(
                            "prompt already in progress".to_string(),
                        )));
                    }
                }
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

    let namespaced_name = matching_prompt.name.to_string();

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
            rmcp::model::PromptMessageContent::Text { text } => Ok(text.to_string()),
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
            (
                "1".to_string(),
                serde_json::Value::String("do".to_string()),
            ),
            (
                "2".to_string(),
                serde_json::Value::String("a".to_string()),
            ),
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
}

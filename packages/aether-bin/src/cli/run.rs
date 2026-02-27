use aether::core::{Prompt, agent};
use aether::events::{AgentMessage, UserMessage};
use aether::mcp::McpSpawnResult;
use aether::mcp::mcp;
use mcp_servers::McpBuilderExt;
use std::io;
use std::process::ExitCode;
use tracing::debug;

use super::error::CliError;
use super::{Cli, OutputFormat};
use crate::prompt::build_system_prompt;

pub async fn run(cli: Cli) -> Result<ExitCode, CliError> {
    setup_tracing(cli.verbose, &cli.output);

    let prompt = cli.resolve_prompt()?;
    let llm = cli.resolve_model()?;
    let cwd = cli.cwd.canonicalize().map_err(CliError::IoError)?;
    debug!("Working directory: {}", cwd.display());

    let mut builder = mcp().with_builtin_servers(cwd.clone(), &cwd);
    let mcp_config_path = cli
        .mcp_config
        .or_else(|| cwd.join("mcp.json").exists().then(|| cwd.join("mcp.json")));

    if let Some(ref config_path) = mcp_config_path {
        debug!("Loading MCP config from: {}", config_path.display());
        let config_str = config_path
            .to_str()
            .ok_or_else(|| CliError::McpError("Invalid MCP config path".to_string()))?;

        builder = builder
            .from_json_file(config_str)
            .await
            .map_err(|e| CliError::McpError(e.to_string()))?;
    }

    let McpSpawnResult {
        tool_definitions,
        instructions,
        command_tx: mcp_tx,
        elicitation_rx: _,
        handle: _mcp_handle,
    } = builder
        .spawn()
        .await
        .map_err(|e| CliError::McpError(e.to_string()))?;

    let system_prompt = build_system_prompt(&cwd, instructions, cli.system_prompt.as_deref())
        .await
        .map_err(CliError::AgentError)?;

    let agent_builder = agent(llm)
        .system_prompt(Prompt::text(&system_prompt))
        .tools(mcp_tx, tool_definitions);

    let (agent_tx, agent_rx, _agent_handle) = agent_builder
        .spawn()
        .await
        .map_err(|e| CliError::AgentError(e.to_string()))?;

    agent_tx
        .send(UserMessage::text(&prompt))
        .await
        .map_err(|e| CliError::AgentError(format!("Failed to send prompt: {e}")))?;

    Ok(stream_output(agent_rx, &cli.output).await)
}

async fn stream_output(
    mut rx: tokio::sync::mpsc::Receiver<AgentMessage>,
    format: &OutputFormat,
) -> ExitCode {
    while let Some(msg) = rx.recv().await {
        match format {
            OutputFormat::Text => emit_text(&msg),
            OutputFormat::Pretty | OutputFormat::Json => emit_event(&msg),
        }
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }
    ExitCode::SUCCESS
}

fn format_text(msg: &AgentMessage) -> Option<String> {
    match msg {
        AgentMessage::Text {
            chunk,
            is_complete: true,
            ..
        } => Some(chunk.clone()),

        AgentMessage::Thought {
            chunk,
            is_complete: true,
            ..
        } => Some(format!("Thought: {chunk}")),

        AgentMessage::ToolCall { request, .. } if !request.name.is_empty() => Some(format!(
            "Tool call: {}({})",
            request.name, request.arguments
        )),

        AgentMessage::ToolResult { result, .. } => {
            Some(format!("Tool result [{}]: {}", result.name, result.result))
        }

        AgentMessage::ToolError { error, .. } => {
            Some(format!("Tool error [{}]: {}", error.name, error.error))
        }

        AgentMessage::Error { message } => Some(format!("Error: {message}")),

        AgentMessage::Cancelled { message } => Some(format!("Cancelled: {message}")),

        AgentMessage::AutoContinue {
            attempt,
            max_attempts,
        } => Some(format!("Continuing ({attempt}/{max_attempts})...")),

        AgentMessage::ModelSwitched { previous, new } => {
            Some(format!("Model switched: {previous} -> {new}"))
        }

        _ => None,
    }
}

fn emit_text(msg: &AgentMessage) {
    if let Some(text) = format_text(msg) {
        if matches!(msg, AgentMessage::Error { .. }) {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    }
}

fn emit_event(msg: &AgentMessage) {
    match msg {
        AgentMessage::Text {
            chunk,
            is_complete: true,
            ..
        } => tracing::info!(target: "agent", "{chunk}"),

        AgentMessage::Thought {
            chunk,
            is_complete: true,
            ..
        } => tracing::info!(target: "agent", thought = %chunk),

        AgentMessage::ToolCall { request, .. } if !request.name.is_empty() => {
            tracing::info!(target: "agent", tool = %request.name, arguments = %request.arguments);
        }

        AgentMessage::ToolResult { result, .. } => {
            tracing::info!(target: "agent", tool = %result.name, result = %result.result);
        }

        AgentMessage::ToolError { error, .. } => {
            tracing::warn!(target: "agent", tool = %error.name, error = %error.error);
        }

        AgentMessage::Error { message } => tracing::error!(target: "agent", "{message}"),

        AgentMessage::Cancelled { message } => {
            tracing::info!(target: "agent", cancelled = %message);
        }

        AgentMessage::AutoContinue {
            attempt,
            max_attempts,
        } => tracing::info!(target: "agent", "Continuing ({attempt}/{max_attempts})..."),

        AgentMessage::ModelSwitched { previous, new } => {
            tracing::info!(target: "agent", "Model switched: {previous} -> {new}");
        }

        _ => {}
    }
}

fn setup_tracing(verbose: bool, format: &OutputFormat) {
    use tracing_subscriber::Layer;
    use tracing_subscriber::filter::{self, EnvFilter};
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let diag_filter = if verbose {
        EnvFilter::new("debug,agent=off")
    } else {
        EnvFilter::new("warn,agent=off")
    };

    let diag_layer = fmt::layer()
        .with_writer(io::stderr)
        .with_filter(diag_filter);

    let agent_filter = filter::filter_fn(|meta| meta.target().starts_with("agent"));

    match format {
        OutputFormat::Text => {
            // Text mode writes directly to stdout — no agent tracing layer needed.
            tracing_subscriber::registry().with(diag_layer).init();
        }
        OutputFormat::Pretty => {
            let agent_layer = fmt::layer()
                .with_writer(io::stdout)
                .pretty()
                .with_filter(agent_filter);
            tracing_subscriber::registry()
                .with(diag_layer)
                .with(agent_layer)
                .init();
        }
        OutputFormat::Json => {
            let agent_layer = fmt::layer()
                .with_writer(io::stdout)
                .json()
                .with_filter(agent_filter);
            tracing_subscriber::registry()
                .with(diag_layer)
                .with(agent_layer)
                .init();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::Layer;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt;

    fn with_test_subscriber<F: FnOnce()>(f: F) -> String {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let buf_clone = Arc::clone(&buf);

        let writer = move || -> TestWriter {
            TestWriter {
                buf: Arc::clone(&buf_clone),
            }
        };

        let layer = fmt::layer()
            .with_writer(writer)
            .with_ansi(false)
            .with_level(false)
            .with_target(false)
            .with_timer(fmt::time::uptime())
            .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                meta.target().starts_with("agent")
            }));

        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, f);

        let bytes = buf.lock().unwrap();
        String::from_utf8(bytes.clone()).unwrap()
    }

    #[derive(Clone)]
    struct TestWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl io::Write for TestWriter {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.buf.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    // --- emit_event tests (Pretty/Json mode) ---

    #[test]
    fn emit_event_emits_complete_text() {
        let output = with_test_subscriber(|| {
            emit_event(&AgentMessage::text("id", "hello", true, "model"));
        });
        assert!(output.contains("hello"), "expected 'hello' in: {output}");
    }

    #[test]
    fn emit_event_skips_incomplete_text() {
        let output = with_test_subscriber(|| {
            emit_event(&AgentMessage::text("id", "hello", false, "model"));
        });
        assert!(output.is_empty(), "expected empty output, got: {output}");
    }

    #[test]
    fn emit_event_emits_complete_thought() {
        let output = with_test_subscriber(|| {
            emit_event(&AgentMessage::thought("id", "deep thinking", true, "model"));
        });
        assert!(
            output.contains("deep thinking"),
            "expected 'deep thinking' in: {output}"
        );
    }

    #[test]
    fn emit_event_skips_incomplete_thought() {
        let output = with_test_subscriber(|| {
            emit_event(&AgentMessage::thought("id", "partial", false, "model"));
        });
        assert!(output.is_empty(), "expected empty output, got: {output}");
    }

    #[test]
    fn emit_event_emits_tool_call() {
        let msg = AgentMessage::ToolCall {
            request: llm::ToolCallRequest {
                id: "tc1".to_string(),
                name: "bash".to_string(),
                arguments: "{}".to_string(),
            },
            model_name: "test".to_string(),
        };
        let output = with_test_subscriber(|| {
            emit_event(&msg);
        });
        assert!(output.contains("bash"), "expected 'bash' in: {output}");
    }

    #[test]
    fn emit_event_skips_arg_streaming_tool_call() {
        let msg = AgentMessage::ToolCall {
            request: llm::ToolCallRequest {
                id: "tc1".to_string(),
                name: String::new(),
                arguments: "{\"partial".to_string(),
            },
            model_name: "test".to_string(),
        };
        let output = with_test_subscriber(|| {
            emit_event(&msg);
        });
        assert!(output.is_empty(), "expected empty output, got: {output}");
    }

    #[test]
    fn emit_event_emits_tool_result() {
        let msg = AgentMessage::ToolResult {
            result: llm::ToolCallResult {
                id: "tc1".to_string(),
                name: "bash".to_string(),
                arguments: "{}".to_string(),
                result: "ok".to_string(),
            },
            result_meta: None,
            model_name: "test".to_string(),
        };
        let output = with_test_subscriber(|| {
            emit_event(&msg);
        });
        assert!(output.contains("bash"), "expected 'bash' in: {output}");
        assert!(output.contains("ok"), "expected 'ok' in: {output}");
    }

    #[test]
    fn emit_event_emits_error() {
        let msg = AgentMessage::Error {
            message: "something broke".to_string(),
        };
        let output = with_test_subscriber(|| {
            emit_event(&msg);
        });
        assert!(
            output.contains("something broke"),
            "expected 'something broke' in: {output}"
        );
    }

    #[test]
    fn emit_event_skips_done() {
        let output = with_test_subscriber(|| {
            emit_event(&AgentMessage::Done);
        });
        assert!(output.is_empty(), "expected empty output, got: {output}");
    }

    // --- emit_text tests (Text mode) ---

    #[test]
    fn emit_text_formats_complete_text() {
        assert_eq!(
            format_text(&AgentMessage::text("id", "hello world", true, "m")),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn emit_text_skips_incomplete_text() {
        assert_eq!(
            format_text(&AgentMessage::text("id", "partial", false, "m")),
            None
        );
    }

    #[test]
    fn emit_text_formats_complete_thought() {
        assert_eq!(
            format_text(&AgentMessage::thought("id", "reasoning here", true, "m")),
            Some("Thought: reasoning here".to_string())
        );
    }

    #[test]
    fn emit_text_skips_incomplete_thought() {
        assert_eq!(
            format_text(&AgentMessage::thought("id", "partial", false, "m")),
            None
        );
    }

    #[test]
    fn emit_text_formats_tool_call() {
        let msg = AgentMessage::ToolCall {
            request: llm::ToolCallRequest {
                id: "tc1".to_string(),
                name: "bash".to_string(),
                arguments: r#"{"cmd":"ls"}"#.to_string(),
            },
            model_name: "test".to_string(),
        };
        assert_eq!(
            format_text(&msg),
            Some(r#"Tool call: bash({"cmd":"ls"})"#.to_string())
        );
    }

    #[test]
    fn emit_text_skips_arg_streaming_tool_call() {
        let msg = AgentMessage::ToolCall {
            request: llm::ToolCallRequest {
                id: "tc1".to_string(),
                name: String::new(),
                arguments: "partial".to_string(),
            },
            model_name: "test".to_string(),
        };
        assert_eq!(format_text(&msg), None);
    }

    #[test]
    fn emit_text_formats_tool_result() {
        let msg = AgentMessage::ToolResult {
            result: llm::ToolCallResult {
                id: "tc1".to_string(),
                name: "bash".to_string(),
                arguments: "{}".to_string(),
                result: "output".to_string(),
            },
            result_meta: None,
            model_name: "test".to_string(),
        };
        assert_eq!(
            format_text(&msg),
            Some("Tool result [bash]: output".to_string())
        );
    }

    #[test]
    fn emit_text_formats_tool_error() {
        let msg = AgentMessage::ToolError {
            error: llm::ToolCallError {
                id: "tc1".to_string(),
                name: "bash".to_string(),
                arguments: None,
                error: "not found".to_string(),
            },
            model_name: "test".to_string(),
        };
        assert_eq!(
            format_text(&msg),
            Some("Tool error [bash]: not found".to_string())
        );
    }

    #[test]
    fn emit_text_formats_error() {
        let msg = AgentMessage::Error {
            message: "boom".to_string(),
        };
        assert_eq!(format_text(&msg), Some("Error: boom".to_string()));
    }

    #[test]
    fn emit_text_formats_cancelled() {
        let msg = AgentMessage::Cancelled {
            message: "user stopped".to_string(),
        };
        assert_eq!(
            format_text(&msg),
            Some("Cancelled: user stopped".to_string())
        );
    }

    #[test]
    fn emit_text_formats_auto_continue() {
        let msg = AgentMessage::AutoContinue {
            attempt: 2,
            max_attempts: 5,
        };
        assert_eq!(format_text(&msg), Some("Continuing (2/5)...".to_string()));
    }

    #[test]
    fn emit_text_formats_model_switched() {
        let msg = AgentMessage::ModelSwitched {
            previous: "old-model".to_string(),
            new: "new-model".to_string(),
        };
        assert_eq!(
            format_text(&msg),
            Some("Model switched: old-model -> new-model".to_string())
        );
    }

    #[test]
    fn emit_text_skips_done() {
        assert_eq!(format_text(&AgentMessage::Done), None);
    }
}

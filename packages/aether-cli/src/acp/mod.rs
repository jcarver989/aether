pub(crate) mod config_setting;
pub(crate) mod handlers;
pub(crate) mod mappers;
pub(crate) mod model_config;
pub(crate) mod relay;
pub(crate) mod session;
pub(crate) mod session_manager;
pub(crate) mod session_registry;
pub(crate) mod session_store;
#[cfg(test)]
pub(crate) mod testing;

pub use mappers::map_mcp_prompt_to_available_command;
pub use session_manager::SessionManager;

use acp_utils::server::AcpConnectionHandle;
use agent_client_protocol::{self as acp, ByteStreams, Client, ConnectionTo, ErrorCode};
use std::sync::Arc;
use std::{fs::create_dir_all, path::PathBuf};
use tokio::io::{stdin, stdout};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::info;
use tracing_appender::rolling::daily;
use tracing_subscriber::EnvFilter;

#[derive(clap::Args, Debug)]
pub struct AcpArgs {
    /// Path to log file directory (default: /tmp/aether-acp-logs)
    #[clap(long, default_value = "/tmp/aether-acp-logs")]
    pub log_dir: PathBuf,
}

/// Outcome of running the ACP server successfully.
#[derive(Debug)]
pub enum AcpRunOutcome {
    /// The client disconnected cleanly (e.g. EOF on stdin).
    CleanDisconnect,
}

/// Errors that terminate the ACP server run.
#[derive(Debug)]
pub enum AcpRunError {
    Protocol(acp::Error),
}

impl std::fmt::Display for AcpRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcpRunError::Protocol(e) => write!(f, "ACP protocol error: {e}"),
        }
    }
}

impl std::error::Error for AcpRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AcpRunError::Protocol(e) => Some(e),
        }
    }
}

pub async fn run_acp(args: AcpArgs) -> Result<AcpRunOutcome, AcpRunError> {
    info!("Starting Aether ACP server");

    setup_logging(&args);

    let connection = AcpConnectionHandle::new_disconnected();
    let manager = Arc::new(SessionManager::new(Arc::new(connection.clone())));

    let transport = ByteStreams::new(stdout().compat_write(), stdin().compat());

    let connect_result = handlers::configure_agent_builder(manager.clone())
        .connect_with(transport, {
            let connection = connection.clone();
            async move |cx: ConnectionTo<Client>| {
                connection.attach(cx);
                std::future::pending::<()>().await;
                Ok(())
            }
        })
        .await;

    manager.shutdown_all_sessions().await;

    classify_run_result(connect_result)
}

fn classify_run_result(result: Result<(), acp::Error>) -> Result<AcpRunOutcome, AcpRunError> {
    match result {
        Ok(()) => Ok(AcpRunOutcome::CleanDisconnect),
        Err(err) if is_transport_closed(&err) => Ok(AcpRunOutcome::CleanDisconnect),
        Err(err) => Err(AcpRunError::Protocol(err)),
    }
}

/// `connect_with` surfaces client-side transport close (stdin EOF, SIGPIPE on
/// stdout) as an error while the foreground future is still pending. The ACP
/// SDK wraps any `io::Error` that bubbles up from the transport actor via
/// [`acp::Error::into_internal_error`], which stamps the code to
/// [`ErrorCode::InternalError`] and moves the stringified IO error into the
/// `data` field. We inspect `data` instead of `to_string()` because the
/// per-kind `Display` strings in `std::io::Error` are stable across Rust
/// releases, whereas the top-level `Error::Display` prefix is implementation
/// detail of the SDK.
fn is_transport_closed(err: &acp::Error) -> bool {
    if err.code != ErrorCode::InternalError {
        return false;
    }
    let Some(data) = err.data.as_ref().and_then(serde_json::Value::as_str) else {
        return false;
    };
    // io::Error Display strings for the closed-transport kinds we care about.
    matches!(
        data,
        "broken pipe" | "connection reset" | "unexpected end of file" | "failed to fill whole buffer" | "early eof"
    )
}

fn setup_logging(args: &AcpArgs) {
    create_dir_all(&args.log_dir).ok();
    tracing_subscriber::fmt()
        .with_writer(daily(&args.log_dir, "aether-acp.log"))
        .with_ansi(false) // No ANSI colors in log files
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .pretty()
        .init();
}

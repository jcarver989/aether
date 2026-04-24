pub(crate) mod config_setting;
pub(crate) mod handlers;
pub(crate) mod mappers;
pub(crate) mod model_config;
pub(crate) mod relay;
pub(crate) mod session;
pub(crate) mod session_manager;
pub(crate) mod session_registry;
pub(crate) mod session_store;

pub use mappers::map_mcp_prompt_to_available_command;
pub use session_manager::SessionManager;

use agent_client_protocol::{self as acp, ByteStreams};
use std::sync::Arc;
use std::{fs::create_dir_all, path::PathBuf};
use tokio::io::{stdin, stdout};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::info;
use tracing_appender::rolling::daily;
use tracing_subscriber::EnvFilter;

use crate::acp::handlers::acp_agent_builder;

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

    let manager = Arc::new(SessionManager::new());

    let transport = ByteStreams::new(stdout().compat_write(), stdin().compat());
    let connect_result = handlers::acp_agent_builder(manager.clone()).connect_to(transport).await;

    manager.shutdown_all_sessions().await;

    match connect_result {
        Ok(()) => Ok(AcpRunOutcome::CleanDisconnect),
        Err(err) => Err(AcpRunError::Protocol(err)),
    }
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

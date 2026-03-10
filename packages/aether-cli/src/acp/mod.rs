pub(crate) mod config_setting;
pub mod mappers;
pub(crate) mod model_config;
pub(crate) mod relay;
pub(crate) mod session;
pub(crate) mod session_manager;
pub(crate) mod settings;

pub use mappers::map_mcp_prompt_to_available_command;
pub use session_manager::SessionManager;

use acp_utils::server::{AcpActor, AcpActorHandle};
use agent_client_protocol::{self as acp, AgentSideConnection};
use std::{fs::create_dir_all, path::PathBuf};
use tokio::{
    io::{stdin, stdout},
    sync::mpsc,
    task::{LocalSet, spawn_local},
};
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

pub async fn run_acp(args: AcpArgs) -> acp::Result<()> {
    info!("Starting Aether ACP server");

    setup_logging(&args);
    let stdout = stdout().compat_write();
    let stdin = stdin().compat();

    // Use multi-threaded runtime with LocalSet:
    // - LocalSet pins !Send ACP futures to this thread via spawn_local
    // - The multi-threaded pool allows agents to use tokio::spawn for Send futures
    // This enables sub-agents to spawn correctly while supporting ACP's !Send requirements
    LocalSet::new()
        .run_until(async move {
            let (actor_request_tx, actor_request_rx) = mpsc::unbounded_channel();
            let actor_handle = AcpActorHandle::new(actor_request_tx);
            let agent = SessionManager::new(actor_handle.clone());

            let (conn, handle_io) = AgentSideConnection::new(agent, stdout, stdin, |fut| {
                spawn_local(fut);
            });

            let actor = AcpActor::new(conn, actor_request_rx);
            spawn_local(async move {
                actor.run().await;
            });

            handle_io.await
        })
        .await
}

fn setup_logging(args: &AcpArgs) {
    create_dir_all(&args.log_dir).ok();
    tracing_subscriber::fmt()
        .with_writer(daily(&args.log_dir, "aether-acp.log"))
        .with_ansi(false) // No ANSI colors in log files
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .pretty()
        .init();
}

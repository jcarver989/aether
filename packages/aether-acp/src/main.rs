use crate::session_manager::SessionManager;
use acp_utils::server::{AcpActor, AcpActorHandle};
use agent_client_protocol::{self as acp, AgentSideConnection};
use clap::Parser;
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

mod mappers;
mod session;
mod session_manager;

#[derive(Parser, Debug)]
#[clap(name = "aether-acp", about = "Aether Agent Client Protocol Server")]
struct Args {
    /// Model provider in the format "provider:model" (e.g., "anthropic:claude-3.5-sonnet", "ollama:llama3.2")
    #[clap(long, default_value = "zai:GLM-4.6")]
    model: String,

    /// System prompt for the agent
    #[clap(long)]
    system_prompt: Option<String>,

    /// Path to MCP configuration file
    #[clap(long)]
    mcp_config: PathBuf,

    /// Path to log file directory (default: /tmp/aether-acp-logs)
    #[clap(long, default_value = "/tmp/aether-acp-logs")]
    log_dir: PathBuf,
}

#[tokio::main]
async fn main() -> acp::Result<()> {
    let args = Args::parse();
    info!("Starting Aether ACP server");
    info!("Model: {}", args.model);
    info!("MCP config: {:?}", args.mcp_config);

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
            let agent = SessionManager::new(
                args.model,
                args.system_prompt,
                args.mcp_config,
                actor_handle.clone(),
            );

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

fn setup_logging(args: &Args) -> () {
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

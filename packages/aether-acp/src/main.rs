use agent_client_protocol as acp;
use clap::Parser;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::info;

use crate::session_manager::SessionManager;
use acp_utils::server::{AcpActor, AcpActorHandle};

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

    // Initialize tracing subscriber for logging to file
    // IMPORTANT: Write to file, not stdout/stderr, since stdout is used for ACP JSON-RPC communication
    std::fs::create_dir_all(&args.log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(&args.log_dir, "aether-acp.log");
    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_ansi(false) // No ANSI colors in log files
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .pretty()
        .init();

    info!("Starting Aether ACP server");
    info!("Model: {}", args.model);
    info!("MCP config: {:?}", args.mcp_config);

    let stdout = tokio::io::stdout().compat_write();
    let stdin = tokio::io::stdin().compat();

    info!("Using MCP config path: {:?}", args.mcp_config);

    // Use multi-threaded runtime with LocalSet:
    // - LocalSet pins !Send ACP futures to this thread via spawn_local
    // - The multi-threaded pool allows agents to use tokio::spawn for Send futures
    // This enables sub-agents to spawn correctly while supporting ACP's !Send requirements
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            // Create channel for ACP actor
            let (actor_request_tx, actor_request_rx) = mpsc::unbounded_channel();
            let actor_handle = AcpActorHandle::new(actor_request_tx);

            let agent = SessionManager::new(
                args.model,
                args.system_prompt,
                args.mcp_config,
                actor_handle.clone(),
            );

            let (conn, handle_io) = acp::AgentSideConnection::new(agent, stdout, stdin, |fut| {
                tokio::task::spawn_local(fut);
            });

            // Spawn the ACP actor with owned connection
            let actor = AcpActor::new(conn, actor_request_rx);
            tokio::task::spawn_local(async move {
                actor.run().await;
            });

            // Run until stdin/stdout are closed
            handle_io.await
        })
        .await
}

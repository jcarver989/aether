use aether_acp::SessionManager;
use agent_client_protocol as acp;
use agent_client_protocol::Client;
use clap::Parser;
use std::path::PathBuf;
use std::rc::Rc;
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::{error, info};
use tracing_subscriber;
use tracing_appender;

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

#[tokio::main(flavor = "current_thread")]
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
        .init();

    info!("Starting Aether ACP server");
    info!("Model: {}", args.model);
    info!("MCP config: {:?}", args.mcp_config);

    let stdout = tokio::io::stdout().compat_write();
    let stdin = tokio::io::stdin().compat();

    info!("Using MCP config path: {:?}", args.mcp_config);

    // The AgentSideConnection will spawn futures onto our Tokio runtime.
    // LocalSet and spawn_local are used because the futures from the
    // agent-client-protocol crate are not Send.
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            let (notification_tx, mut notification_rx) = mpsc::unbounded_channel();

            let agent = SessionManager::new(
                args.model,
                args.system_prompt,
                args.mcp_config,
                notification_tx,
            );

            let (conn, handle_io) = acp::AgentSideConnection::new(agent, stdout, stdin, |fut| {
                tokio::task::spawn_local(fut);
            });

            // Wrap connection in Rc for sharing
            let conn = Rc::new(conn);

            let conn_clone = Rc::clone(&conn);
            tokio::task::spawn_local(async move {
                while let Some((session_notification, tx)) = notification_rx.recv().await {
                    let result = conn_clone.session_notification(session_notification).await;
                    if let Err(e) = result {
                        error!("Failed to send session notification: {}", e);
                        break;
                    }
                    tx.send(()).ok();
                }
            });

            // Run until stdin/stdout are closed
            handle_io.await
        })
        .await
}

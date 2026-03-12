use clap::Parser;
use mcp_utils::ServiceExt;
use rmcp::ServerHandler;
use rmcp::transport::io::stdio;

#[derive(Parser)]
#[command(name = "mcp-servers-stdio", about = "Run an MCP server over stdio")]
struct Cli {
    /// Which server to run: coding, lsp, skills, tasks, subagents, survey
    #[arg(long)]
    server: String,

    /// Arguments forwarded to the selected server (e.g. --root-dir /path)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Debug)]
enum StdioError {
    UnknownServer(String),
    ServerArgs(String),
    Serve(String),
    Join(tokio::task::JoinError),
}

impl std::fmt::Display for StdioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StdioError::UnknownServer(name) => {
                write!(
                    f,
                    "Unknown server: '{name}'. Available: coding, lsp, skills, tasks, subagents, survey"
                )
            }
            StdioError::ServerArgs(msg) => write!(f, "{msg}"),
            StdioError::Serve(msg) => write!(f, "Failed to start server: {msg}"),
            StdioError::Join(e) => write!(f, "Server task failed: {e}"),
        }
    }
}

async fn serve_stdio(server: impl ServerHandler) -> Result<(), StdioError> {
    let running = server
        .serve(stdio())
        .await
        .map_err(|e| StdioError::Serve(e.to_string()))?;
    running.waiting().await.map_err(StdioError::Join)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), StdioError> {
    let cli = Cli::parse();

    match cli.server.as_str() {
        "coding" => {
            let parsed =
                mcp_servers::CodingMcpArgs::from_args(cli.args).map_err(StdioError::ServerArgs)?;
            let server = mcp_servers::CodingMcp::new();
            let server = if let Some(root_dir) = parsed.root_dir {
                server.with_lsp(root_dir.clone()).with_root_dir(root_dir)
            } else {
                server
            };
            serve_stdio(server).await
        }
        "lsp" => {
            let server =
                mcp_servers::LspMcp::from_args(cli.args).map_err(StdioError::ServerArgs)?;
            serve_stdio(server).await
        }
        "skills" => {
            let server =
                mcp_servers::SkillsMcp::from_args(cli.args).map_err(StdioError::ServerArgs)?;
            serve_stdio(server).await
        }
        "tasks" => {
            let server =
                mcp_servers::TasksMcp::from_args(cli.args).map_err(StdioError::ServerArgs)?;
            serve_stdio(server).await
        }
        "subagents" => {
            let server =
                mcp_servers::SubAgentsMcp::from_args(cli.args).map_err(StdioError::ServerArgs)?;
            serve_stdio(server).await
        }
        "survey" => {
            let server =
                mcp_servers::SurveyMcp::from_args(cli.args).map_err(StdioError::ServerArgs)?;
            serve_stdio(server).await
        }
        other => Err(StdioError::UnknownServer(other.to_string())),
    }
}

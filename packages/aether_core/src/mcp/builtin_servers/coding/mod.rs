use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

pub mod bash;
pub mod common;
pub mod find;
pub mod grep;
pub mod read_file;
pub mod write_file;

pub use bash::{BashArgs, execute_command};
pub use find::{FindArgs, find_files_by_name};
pub use grep::{GrepArgs, perform_grep};
pub use read_file::{ReadFileArgs, read_file_contents};
pub use write_file::{WriteFileArgs, write_file_contents};

#[derive(Debug, Clone)]
pub struct CodingMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for CodingMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "coding-mcp".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "A coding MCP server with grep-powered search, file operations (read/write), and bash command execution capabilities".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl CodingMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Search for patterns in file contents using ripgrep. Use output_mode 'matches' for matching lines or 'files_only' for filenames. Specify file_path to search a single file.")]
    pub async fn grep(&self, request: Parameters<GrepArgs>) -> String {
        let Parameters(args) = request;

        match perform_grep(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Grep error: {}", e),
        }
    }

    #[tool(
        description = "Find files by filename pattern (supports wildcards like *.rs, main.*, etc.)"
    )]
    pub async fn find(&self, request: Parameters<FindArgs>) -> String {
        let Parameters(args) = request;

        match find_files_by_name(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Find error: {}", e),
        }
    }

    #[tool(description = "Read contents of a file")]
    pub async fn read_file(&self, request: Parameters<ReadFileArgs>) -> String {
        let Parameters(args) = request;

        match read_file_contents(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Read file error: {}", e),
        }
    }

    #[tool(description = "Write content to a file")]
    pub async fn write_file(&self, request: Parameters<WriteFileArgs>) -> String {
        let Parameters(args) = request;

        match write_file_contents(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Write file error: {}", e),
        }
    }

    #[tool(description = "Execute a bash command")]
    pub async fn bash(&self, request: Parameters<BashArgs>) -> String {
        let Parameters(args) = request;

        match execute_command(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Bash command error: {}", e),
        }
    }
}
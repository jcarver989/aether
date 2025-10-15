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
pub mod list_files;
pub mod read_file;
pub mod write_file;

pub use bash::{BashArgs, execute_command};
pub use find::{FindArgs, find_files_by_name};
pub use grep::{GrepArgs, perform_grep};
pub use list_files::{ListFilesArgs, list_files};
pub use read_file::{ReadFileArgs, read_file_contents};
pub use write_file::{WriteFileProps, write_file_contents};

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

    #[tool(
        description = "Search for patterns in file contents using ripgrep with advanced features. Supports context lines (-A, -B, -C), file type filtering (--type rust, python, etc.), max results limiting, inverse matching, and word boundary matching. Use output_mode 'matches' for matching lines or 'files_only' for filenames."
    )]
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

    #[tool(
        description = "Read contents of a file with line numbers. Use 'offset' to start from specific line (1-indexed) and 'limit' to specify max lines. Defaults to reading entire file. Returns content formatted as '   1│ line content' for easy reference with write_file operations."
    )]
    pub async fn read_file(&self, request: Parameters<ReadFileArgs>) -> String {
        let Parameters(args) = request;

        match read_file_contents(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Read file error: {}", e),
        }
    }

    #[tool(
        description = "Write content to a file using operations. Supports 'overwrite' (replace entire file) and 'line_range' (replace/insert/append specific lines using 1-indexed line numbers from read_file). Operations are applied sequentially."
    )]
    pub async fn write_file(&self, request: Parameters<WriteFileProps>) -> String {
        let Parameters(args) = request;

        match write_file_contents(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("Write file error: {}", e),
        }
    }

    #[tool(description = "List files and directories in a specified path")]
    pub async fn list_files(&self, request: Parameters<ListFilesArgs>) -> String {
        let Parameters(args) = request;

        match list_files(args).await {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "Error serializing result".to_string()),
            Err(e) => format!("List files error: {}", e),
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

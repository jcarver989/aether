use super::{
    ElicitationRequest, McpConfigParser, McpError, McpManager, McpServerConfig, ParseError,
    run_mcp_task::{McpCommand, run_mcp_task},
};
use crate::types::ToolDefinition;
use tokio::{
    sync::mpsc::{self, Sender},
    task::JoinHandle,
};

pub fn mcp() -> McpBuilder {
    McpBuilder::new()
}

pub struct McpBuilder {
    mcp_configs: Vec<McpServerConfig>,
}

impl McpBuilder {
    pub fn new() -> Self {
        Self {
            mcp_configs: Vec::new(),
        }
    }

    pub fn add(mut self, configs: Vec<McpServerConfig>) -> Self {
        self.mcp_configs.extend(configs);
        self
    }

    pub fn mcp_json_file(mut self, path: &str) -> Result<Self, ParseError> {
        let mcp_configs = McpConfigParser::new().parse_json_file(path)?;
        self.mcp_configs.extend(mcp_configs);
        Ok(self)
    }

    pub async fn spawn(
        self,
    ) -> Result<(Vec<ToolDefinition>, Sender<McpCommand>, JoinHandle<()>), McpError> {
        let (mcp_command_tx, mcp_command_rx) = mpsc::channel::<McpCommand>(100);
        let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(100);

        let mut mcp_manager = McpManager::new(elicitation_tx);
        mcp_manager.add_mcps(self.mcp_configs).await?;
        let tool_definitions = mcp_manager.tool_definitions();
        let mcp_handle = tokio::spawn(run_mcp_task(mcp_manager, mcp_command_rx));

        Ok((tool_definitions, mcp_command_tx, mcp_handle))
    }
}

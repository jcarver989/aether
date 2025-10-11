use rmcp::{
    Json, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    service::DynService,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::McpServerConfig;

pub fn fake_mcp(name: &str, server: FakeMcpServer) -> McpServerConfig {
    McpServerConfig::InMemory {
        name: name.to_string(),
        server: server.as_dyn(),
    }
}

/// A fake MCP server for testing
#[derive(Clone)]
pub struct FakeMcpServer {
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FakeMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "fake-mcp-server".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some("A fake MCP server for testing".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AddNumbersRequest {
    pub a: u32,
    pub b: u32,
}

impl AddNumbersRequest {
    pub fn new(a: u32, b: u32) -> Self {
        Self { a, b }
    }

    pub fn json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AddNumbersResult {
    pub sum: u32,
}

impl AddNumbersResult {
    pub fn new(sum: u32) -> Self {
        Self { sum }
    }

    pub fn json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[tool_router]
impl FakeMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    pub fn as_dyn(self) -> Box<dyn DynService<RoleServer>> {
        Box::new(self)
    }

    #[tool(description = "Echoes back the input message")]
    pub async fn add_numbers(
        &self,
        request: Parameters<AddNumbersRequest>,
    ) -> Json<AddNumbersResult> {
        let Parameters(AddNumbersRequest { a, b }) = request;
        Json(AddNumbersResult { sum: a + b })
    }
}

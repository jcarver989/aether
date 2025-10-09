use aether::{
    agent::{AgentMessage, Prompt, UserMessage, agent},
    mcp::config::McpServerConfig,
    testing::FakeLlmProvider,
    types::LlmResponse,
};
use rmcp::ServiceExt;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Simple test tools for parallel execution testing
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DelayArgs {
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TestMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TestMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: rmcp::model::Implementation {
                name: "test_mcp".into(),
                version: "1.0.0".into(),
                title: None,
                icons: None,
                website_url: None,
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl TestMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "A test tool that simulates work with a delay")]
    pub async fn delay_tool(&self, request: Parameters<DelayArgs>) -> String {
        let Parameters(args) = request;
        tokio::time::sleep(tokio::time::Duration::from_millis(args.duration_ms)).await;
        format!("Completed delay of {}ms", args.duration_ms)
    }
}

#[tokio::test]
async fn test_basic_functionality_still_works() {
    // Simple test to verify basic functionality still works
    let responses = vec![
        LlmResponse::Start {
            message_id: "msg_1".to_string(),
        },
        LlmResponse::Text {
            chunk: "Hello! ".to_string(),
        },
        LlmResponse::Text {
            chunk: "How can I help you?".to_string(),
        },
        LlmResponse::Done,
    ];

    let llm = FakeLlmProvider::with_single_response(responses);

    let mut agent = agent(llm)
        .system(&Prompt::text("You are a test assistant").build().unwrap())
        .mcp(McpServerConfig::InMemory {
            name: "test_mcp".to_string(),
            server: TestMcp::new().into_dyn(),
        })
        .spawn()
        .await
        .unwrap();

    agent.send(UserMessage::text("Hello")).await.unwrap();

    let mut text_chunks = Vec::new();
    while let Some(event) = agent.recv().await {
        match event {
            AgentMessage::Text { chunk, .. } => {
                text_chunks.push(chunk);
            }
            AgentMessage::Done => break,
            _ => {}
        }
    }

    let combined_text: String = text_chunks.join("");
    assert_eq!(combined_text, "Hello! How can I help you?");

    println!("✅ Basic functionality test passed!");
}

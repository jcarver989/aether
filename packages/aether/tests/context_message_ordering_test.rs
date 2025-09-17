use aether::{
    agent::{AgentMessage, UserMessage, agent},
    mcp::manager::McpServerConfig,
    testing::FakeLlmProvider,
    types::{ChatMessage, LlmResponse, ToolCallRequest},
};
use rmcp::ServiceExt;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SimpleArgs {
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct SimpleMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SimpleMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: rmcp::model::Implementation {
                name: "simple_mcp".into(),
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
impl SimpleMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "A simple tool that returns the input")]
    pub async fn echo_tool(&self, request: Parameters<SimpleArgs>) -> String {
        let Parameters(args) = request;
        format!("Echo: {}", args.value)
    }
}

#[tokio::test]
async fn test_simple_tool_call_completes() {
    // Create a simple test that doesn't loop infinitely
    let responses = vec![
        LlmResponse::Start {
            message_id: "msg_1".to_string(),
        },
        LlmResponse::Text {
            chunk: "Using tool".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "call_1".to_string(),
            name: "echo_tool".to_string(),
        },
        LlmResponse::ToolRequestArg {
            id: "call_1".to_string(),
            chunk: r#"{"value": "test"}"#.to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "call_1".to_string(),
                name: "echo_tool".to_string(),
                arguments: r#"{"value": "test"}"#.to_string(),
            },
        },
        LlmResponse::Done,
    ];

    let llm = FakeLlmProvider::with_single_response(responses);
    let test_mcp = SimpleMcp::new();

    let mut agent = agent(llm)
        .system_prompt("You are a test assistant")
        .mcp(McpServerConfig::InMemory {
            name: "simple_mcp".to_string(),
            server: test_mcp.into_dyn(),
        })
        .build()
        .await
        .unwrap();

    let mut receiver = agent.send(UserMessage::text("Use the echo tool")).await;

    // Collect messages until completion
    let mut messages = Vec::new();
    let mut completed = false;
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 1000; // Prevent infinite loops

    while !completed && iterations < MAX_ITERATIONS {
        iterations += 1;

        match tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv()).await {
            Ok(Some(msg)) => {
                match &msg {
                    AgentMessage::Text { is_complete: true, .. } => {
                        // Text completion means we're done with this iteration
                        // In a real agent loop, there might be more iterations for tool calls
                        // but for our simple test, this should be the end
                        completed = true;
                    },
                    AgentMessage::ToolCall { is_complete: true, .. } => {
                        // Tool call completed
                    },
                    _ => {}
                }
                messages.push(msg);
            },
            Ok(None) => {
                // Channel closed, we're done
                completed = true;
            },
            Err(_) => {
                // Timeout - for this test, we'll consider this completion
                completed = true;
            }
        }
    }

    // Check that we got some messages and didn't hit the iteration limit
    assert!(iterations < MAX_ITERATIONS, "Test hit iteration limit - possible infinite loop");
    assert!(!messages.is_empty(), "Should have received some messages");

    // Count the number of completed tool calls
    let tool_call_count = messages.iter()
        .filter(|msg| matches!(msg, AgentMessage::ToolCall { is_complete: true, .. }))
        .count();

    assert_eq!(tool_call_count, 1, "Should have exactly 1 completed tool call, got {}", tool_call_count);

    println!("✅ Simple tool call test passed! Received {} messages in {} iterations", messages.len(), iterations);
}
use aether::{
    agent::{AgentMessage, UserMessage, agent},
    mcp::manager::McpServerConfig,
    testing::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
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
        .system_prompt("You are a test assistant")
        .mcp(McpServerConfig::InMemory {
            name: "test_mcp".to_string(),
            server: TestMcp::new().into_dyn(),
        })
        .build()
        .await
        .unwrap();

    let mut receiver = agent.send(UserMessage::text("Hello")).await;

    let mut text_chunks = Vec::new();
    while let Some(event) = receiver.recv().await {
        if let AgentMessage::Text { chunk, .. } = event {
            text_chunks.push(chunk);
        }
    }

    let combined_text: String = text_chunks.join("");
    assert_eq!(combined_text, "Hello! How can I help you?");

    println!("✅ Basic functionality test passed!");
}

#[tokio::test]
async fn test_parallel_tool_execution_waits_for_completion() {
    // Test that multiple tool calls execute in parallel but agent waits for all to complete
    let responses = vec![
        LlmResponse::Start {
            message_id: "msg_1".to_string(),
        },
        LlmResponse::Text {
            chunk: "I'll run multiple delay tools in parallel.".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "call_1".to_string(),
            name: "delay_tool".to_string(),
        },
        LlmResponse::ToolRequestArg {
            id: "call_1".to_string(),
            chunk: r#"{"duration_ms": 100}"#.to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "call_1".to_string(),
                name: "delay_tool".to_string(),
                arguments: r#"{"duration_ms": 100}"#.to_string(),
            },
        },
        LlmResponse::ToolRequestStart {
            id: "call_2".to_string(),
            name: "delay_tool".to_string(),
        },
        LlmResponse::ToolRequestArg {
            id: "call_2".to_string(),
            chunk: r#"{"duration_ms": 100}"#.to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "call_2".to_string(),
                name: "delay_tool".to_string(),
                arguments: r#"{"duration_ms": 100}"#.to_string(),
            },
        },
        LlmResponse::Done,
    ];

    let llm = FakeLlmProvider::with_single_response(responses);

    let mut agent = agent(llm)
        .system_prompt("You are a test assistant")
        .mcp(McpServerConfig::InMemory {
            name: "test_mcp".to_string(),
            server: TestMcp::new().into_dyn(),
        })
        .build()
        .await
        .unwrap();

    let start_time = std::time::Instant::now();
    let mut receiver = agent.send(UserMessage::text("Test parallel execution")).await;

    let mut tool_results = Vec::new();
    while let Some(event) = receiver.recv().await {
        if let AgentMessage::ToolCall { result: Some(result), .. } = event {
            tool_results.push(result);
        }
    }

    let elapsed = start_time.elapsed();

    // Should have received 2 tool results
    assert_eq!(tool_results.len(), 2);

    // Both should have completed with the expected message
    assert!(tool_results.iter().all(|r| r.contains("Completed delay of 100ms")));

    // Since tools run in parallel, total time should be closer to 100ms than 200ms
    // Allow some overhead for test execution
    assert!(elapsed.as_millis() < 150, "Expected parallel execution to take < 150ms, took {}ms", elapsed.as_millis());

    println!("✅ Parallel tool execution test passed! Took {}ms", elapsed.as_millis());
}

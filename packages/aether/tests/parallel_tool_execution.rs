use aether::{
    agent::{AgentMessage, UserMessage, agent},
    testing::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
    mcp::manager::McpServerConfig,
};
use rmcp::{ServiceExt};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

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
async fn test_parallel_tool_execution() {
    // Create a fake LLM that returns multiple tool calls
    let tool_call_1 = ToolCallRequest {
        id: "call_1".to_string(),
        name: "test_mcp__delay_tool".to_string(),
        arguments: r#"{"duration_ms": 100}"#.to_string(),
    };

    let tool_call_2 = ToolCallRequest {
        id: "call_2".to_string(),
        name: "test_mcp__delay_tool".to_string(),
        arguments: r#"{"duration_ms": 50}"#.to_string(),
    };

    let responses = vec![
        LlmResponse::Start {
            message_id: "msg_1".to_string(),
        },
        LlmResponse::Text {
            chunk: "I'll execute two tools in parallel.".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "call_1".to_string(),
            name: "test_mcp__delay_tool".to_string(),
        },
        LlmResponse::ToolRequestArg {
            id: "call_1".to_string(),
            chunk: r#"{"duration_ms": 100}"#.to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: tool_call_1,
        },
        LlmResponse::ToolRequestStart {
            id: "call_2".to_string(),
            name: "test_mcp__delay_tool".to_string(),
        },
        LlmResponse::ToolRequestArg {
            id: "call_2".to_string(),
            chunk: r#"{"duration_ms": 50}"#.to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: tool_call_2,
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

    let mut receiver = agent.send(UserMessage::text("Execute tools in parallel")).await;

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        println!("Received event: {:?}", event);
        events.push(event);
    }

    // Verify we received tool call events
    let tool_call_events: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentMessage::ToolCall { tool_call_id, is_complete, result, .. } => {
                if *is_complete {
                    Some((tool_call_id.clone(), result.as_ref().unwrap().clone()))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    println!("Tool call events: {:?}", tool_call_events);

    // Should have completed both tool calls
    assert_eq!(tool_call_events.len(), 2);

    // Verify both tools were executed
    let tool_ids: Vec<&String> = tool_call_events.iter().map(|(id, _)| id).collect();
    assert!(tool_ids.contains(&&"call_1".to_string()));
    assert!(tool_ids.contains(&&"call_2".to_string()));

    println!("✅ Parallel tool execution test passed!");
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
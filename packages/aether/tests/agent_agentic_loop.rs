use aether::{
    agent::{AgentMessage, UserMessage, agent},
    mcp::manager::McpServerConfig,
    testing::fake_llm::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
};
use futures::{StreamExt, pin_mut};
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

    #[tool(description = "A simple tool that returns the input")]
    pub async fn echo_tool(&self, request: Parameters<SimpleArgs>) -> String {
        let Parameters(args) = request;
        format!("Echo: {}", args.value)
    }
}

#[tokio::test]
async fn test_simple_tool_execution() {
    // Create a fake LLM that requests a tool call and then responds
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
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
    ]);

    let test_mcp = TestMcp::new();

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .mcp(McpServerConfig::InMemory {
            name: "test_mcp".to_string(),
            server: test_mcp.into_dyn(),
        })
        .build()
        .await
        .unwrap();

    let stream = agent.send(UserMessage::text("Write a test file")).await;
    pin_mut!(stream);

    let mut events = Vec::new();
    let mut completed = false;
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 1000; // Prevent infinite loops

    while !completed && iterations < MAX_ITERATIONS {
        iterations += 1;

        match tokio::time::timeout(std::time::Duration::from_millis(100), stream.next()).await {
            Ok(Some(event)) => {
                match &event {
                    AgentMessage::Text {
                        is_complete: true, ..
                    } => {
                        // Don't end immediately - wait for tool calls to complete
                    }
                    AgentMessage::ToolCall {
                        is_complete: true, ..
                    } => {
                        // Tool call completed - this is what we're waiting for
                        completed = true;
                    }
                    _ => {}
                }
                events.push(event);
            }
            Ok(None) => {
                // Channel closed, we're done
                completed = true;
            }
            Err(_) => {
                // Timeout - for this test, we'll consider this completion after some iterations
                if iterations > 50 {
                    completed = true;
                }
            }
        }
    }

    // Debug output
    println!(
        "Test collected {} events in {} iterations",
        events.len(),
        iterations
    );
    for (i, event) in events.iter().enumerate() {
        println!("  Event {}: {:?}", i, event);
    }

    // Verify we got the expected events
    let content_chunks: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentMessage::Text { chunk, .. } => Some(chunk.as_str()),
            _ => None,
        })
        .collect();

    let tool_calls: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentMessage::ToolCall {
                tool_call_id,
                name,
                is_complete,
                ..
            } => Some((tool_call_id.as_str(), name.as_str(), *is_complete)),
            _ => None,
        })
        .collect();

    assert!(!content_chunks.is_empty());
    assert!(!tool_calls.is_empty());

    // Check that we completed within reasonable time (no infinite loop)
    assert!(
        iterations < MAX_ITERATIONS,
        "Test should complete without infinite loop, took {} iterations", iterations
    );

    // Check that we have tool calls that start
    assert!(
        tool_calls
            .iter()
            .any(|(id, name, _)| *id == "call_1" && *name == "echo_tool")
    );

    // Check that we eventually get tool results
    let tool_results: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentMessage::ToolCall {
                tool_call_id,
                result: Some(result),
                is_complete: true,
                ..
            } => Some((tool_call_id.as_str(), result.as_str())),
            _ => None,
        })
        .collect();

    // We expect to see tool execution results
    assert!(
        !tool_results.is_empty(),
        "Expected to see tool execution results, but got none"
    );
}

#[tokio::test]
async fn test_tool_execution_error_handling() {
    // Create a fake LLM that makes a tool call with invalid arguments
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "tool1".to_string(),
            name: "echo_tool".to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool1".to_string(),
                name: "echo_tool".to_string(),
                arguments: "invalid json".to_string(), // This should cause an error
            },
        },
        LlmResponse::Done,
    ]);

    let test_mcp = TestMcp::new();

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .mcp(McpServerConfig::InMemory {
            name: "test_mcp".to_string(),
            server: test_mcp.into_dyn(),
        })
        .build()
        .await
        .unwrap();

    let stream = agent.send(UserMessage::text("Write a file")).await;
    pin_mut!(stream);

    let mut events = Vec::new();
    let mut completed = false;
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 1000;

    while !completed && iterations < MAX_ITERATIONS {
        iterations += 1;

        match tokio::time::timeout(std::time::Duration::from_millis(100), stream.next()).await {
            Ok(Some(event)) => {
                match &event {
                    AgentMessage::Text {
                        is_complete: true, ..
                    } => {
                        // Don't end immediately - wait for tool calls to complete
                    }
                    AgentMessage::ToolCall {
                        is_complete: true, ..
                    } => {
                        // Tool call completed - this is what we're waiting for
                        completed = true;
                    }
                    _ => {}
                }
                events.push(event);
            }
            Ok(None) => {
                // Channel closed, we're done
                completed = true;
            }
            Err(_) => {
                // Timeout - for this test, we'll consider this completion after some iterations
                if iterations > 50 {
                    completed = true;
                }
            }
        }
    }

    // Debug output
    println!(
        "Error test collected {} events in {} iterations",
        events.len(),
        iterations
    );
    for (i, event) in events.iter().enumerate() {
        println!("  Event {}: {:?}", i, event);
    }

    // Check that we completed within reasonable time (no infinite loop)
    assert!(
        iterations < MAX_ITERATIONS,
        "Test should complete without infinite loop, took {} iterations", iterations
    );

    // Check that we have tool calls with invalid arguments
    let has_tool_calls = events.iter().any(|e| match e {
        AgentMessage::ToolCall {
            tool_call_id, name, ..
        } => tool_call_id == "tool1" && name == "echo_tool",
        _ => false,
    });

    assert!(
        has_tool_calls,
        "Expected to see tool calls with invalid arguments"
    );

    // Check that we get tool execution results (likely error results)
    let tool_results: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentMessage::ToolCall {
                tool_call_id,
                result: Some(result),
                is_complete: true,
                ..
            } => Some((tool_call_id.as_str(), result.as_str())),
            _ => None,
        })
        .collect();

    // We expect to see tool execution results (even if they're errors)
    assert!(
        !tool_results.is_empty(),
        "Expected to see tool execution results (errors), but got none"
    );
}

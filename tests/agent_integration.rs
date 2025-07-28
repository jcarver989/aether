mod utils;

use aether::agent::Agent;
use aether::llm::provider::StreamChunk;
use aether::testing::{FakeLlmProvider, InMemoryFileSystem};
use aether::types::ChatMessage;
use chrono::Utc;
use rmcp::model::Tool as RmcpTool;
use serde_json::{json, Map};
use std::sync::Arc;
use tokio_stream::StreamExt;
use utils::*;

/// Test basic agent creation and initialization
#[tokio::test]
async fn test_agent_creation() {
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("Hello!".to_string()), StreamChunk::Done]);
    let tool_registry = create_test_tool_registry();
    let system_prompt = Some("You are a helpful assistant.".to_string());
    
    let agent = Agent::new(llm_provider, tool_registry, system_prompt);
    
    assert_eq!(agent.conversation_history().len(), 0);
    assert_eq!(agent.llm_provider().call_count(), 0);
}

/// Test agent conversation history management
#[tokio::test]
async fn test_agent_conversation_history() {
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("Hello!".to_string()), StreamChunk::Done]);
    let tool_registry = create_test_tool_registry();
    let mut agent = Agent::new(llm_provider, tool_registry, None);
    
    // Add user message
    let user_msg = ChatMessage::User {
        content: "Hello, how are you?".to_string(),
        timestamp: Utc::now(),
    };
    agent.add_message(user_msg.clone());
    
    // Add assistant message
    let assistant_msg = ChatMessage::Assistant {
        content: "I'm doing well, thank you!".to_string(),
        timestamp: Utc::now(),
    };
    agent.add_message(assistant_msg.clone());
    
    // Verify conversation history
    let history = agent.conversation_history();
    assert_eq!(history.len(), 2);
    
    if let ChatMessage::User { content, .. } = &history[0] {
        assert_eq!(content, "Hello, how are you?");
    } else {
        panic!("Expected user message");
    }
    
    if let ChatMessage::Assistant { content, .. } = &history[1] {
        assert_eq!(content, "I'm doing well, thank you!");
    } else {
        panic!("Expected assistant message");
    }
    
    // Test clearing history
    agent.clear_history();
    assert_eq!(agent.conversation_history().len(), 0);
}

/// Test agent streaming functionality
#[tokio::test]
async fn test_agent_streaming() {
    let chunks = vec![
        StreamChunk::Content("Hello".to_string()),
        StreamChunk::Content(" there!".to_string()),
        StreamChunk::Done,
    ];
    let llm_provider = FakeLlmProvider::with_single_response(chunks);
    let tool_registry = create_test_tool_registry();
    let mut agent = Agent::new(llm_provider, tool_registry, None);
    
    // Add a user message first
    agent.add_message(ChatMessage::User {
        content: "Say hello".to_string(),
        timestamp: Utc::now(),
    });
    
    // Start streaming
    let mut stream = agent.stream_completion(None).await.unwrap();
    
    // Process stream chunks
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.unwrap();
        match chunk {
            StreamChunk::Content(content) => {
                agent.append_streaming_content(&content);
            }
            StreamChunk::Done => {
                agent.finalize_streaming_message();
                break;
            }
            _ => {}
        }
    }
    
    // Verify the final conversation state
    let history = agent.conversation_history();
    assert_eq!(history.len(), 2);
    
    if let ChatMessage::Assistant { content, .. } = &history[1] {
        assert_eq!(content, "Hello there!");
    } else {
        panic!("Expected assistant message, got: {:?}", &history[1]);
    }
}

/// Test agent with tool calls
#[tokio::test]
async fn test_agent_with_tool_calls() {
    let chunks = vec![
        StreamChunk::Content("I'll help you write a file.".to_string()),
        StreamChunk::ToolCallStart {
            id: "call_123".to_string(),
            name: "write_file".to_string(),
        },
        StreamChunk::ToolCallArgument {
            id: "call_123".to_string(),
            argument: r#"{"path": "/tmp/test.txt", "content": "Hello World"}"#.to_string(),
        },
        StreamChunk::ToolCallComplete {
            id: "call_123".to_string(),
        },
        StreamChunk::Done,
    ];
    
    let llm_provider = FakeLlmProvider::with_single_response(chunks);
    let mut tool_registry = create_test_tool_registry();
    
    // Add a mock tool to the registry
    let mut properties = Map::new();
    properties.insert("path".to_string(), json!({"type": "string"}));
    properties.insert("content".to_string(), json!({"type": "string"}));
    
    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), json!(properties));
    schema.insert("required".to_string(), json!(["path", "content"]));
    
    let tool = RmcpTool::new(
        "write_file".to_string(),
        "Write content to a file".to_string(),
        Arc::new(schema),
    );
    tool_registry.register_tool("test_server".to_string(), tool);
    
    let mut agent = Agent::new(llm_provider, tool_registry, None);
    
    // Add user message
    agent.add_message(ChatMessage::User {
        content: "Please write 'Hello World' to /tmp/test.txt".to_string(),
        timestamp: Utc::now(),
    });
    
    // Process stream and build tool calls
    let mut stream = agent.stream_completion(None).await.unwrap();
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.unwrap();
        match chunk {
            StreamChunk::Content(content) => {
                agent.append_streaming_content(&content);
            }
            StreamChunk::ToolCallStart { id, name } => {
                agent.active_tool_calls_mut().insert(id.clone(), aether::agent::PartialToolCall {
                    id: id.clone(),
                    name,
                    arguments: String::new(),
                });
            }
            StreamChunk::ToolCallArgument { id, argument } => {
                if let Some(partial) = agent.active_tool_calls_mut().get_mut(&id) {
                    partial.arguments.push_str(&argument);
                }
            }
            StreamChunk::ToolCallComplete { id } => {
                if let Some(partial) = agent.active_tool_calls_mut().remove(&id) {
                    // Clone arguments before moving the partial
                    let arguments_str = partial.arguments.clone();
                    
                    // Add tool call message
                    agent.add_message(ChatMessage::ToolCall {
                        id: partial.id.clone(),
                        name: partial.name.clone(),
                        params: partial.arguments,
                        timestamp: Utc::now(),
                    });
                    
                    // Record the tool call for loop detection
                    if let Ok(args) = serde_json::from_str::<serde_json::Value>(&arguments_str) {
                        agent.record_tool_call(partial.name, args, Utc::now());
                    }
                }
            }
            StreamChunk::Done => {
                agent.finalize_streaming_message();
                break;
            }
        }
    }
    
    // Verify conversation state
    let history = agent.conversation_history();
    assert_eq!(history.len(), 3);
    
    // Check assistant message (could be Assistant or AssistantStreaming)
    match &history[1] {
        ChatMessage::Assistant { content, .. } => {
            assert_eq!(content, "I'll help you write a file.");
        }
        ChatMessage::AssistantStreaming { content, .. } => {
            assert_eq!(content, "I'll help you write a file.");
        }
        _ => panic!("Expected assistant message, got: {:?}", &history[1]),
    }
    
    // Check tool call message
    if let ChatMessage::ToolCall { name, params, .. } = &history[2] {
        assert_eq!(name, "write_file");
        let parsed_params: serde_json::Value = serde_json::from_str(params).unwrap();
        assert_eq!(parsed_params["path"], "/tmp/test.txt");
        assert_eq!(parsed_params["content"], "Hello World");
    } else {
        panic!("Expected tool call message");
    }
}

/// Test agent tool loop detection
#[tokio::test]
async fn test_agent_tool_loop_detection() {
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("Test response".to_string()), StreamChunk::Done]);
    let tool_registry = create_test_tool_registry();
    let mut agent = Agent::new(llm_provider, tool_registry, None);
    
    let tool_name = "test_tool";
    let arguments = json!({"param": "value"});
    let now = Utc::now();
    
    // Record the same tool call multiple times
    agent.record_tool_call(tool_name.to_string(), arguments.clone(), now);
    agent.record_tool_call(tool_name.to_string(), arguments.clone(), now);
    agent.record_tool_call(tool_name.to_string(), arguments.clone(), now);
    
    // Check for loops
    let loop_count = agent.check_tool_loop(tool_name, &arguments, 5);
    assert_eq!(loop_count, 3);
    
    // Test with different arguments (should not detect loop)
    let different_args = json!({"param": "different_value"});
    let different_loop_count = agent.check_tool_loop(tool_name, &different_args, 5);
    assert_eq!(different_loop_count, 0);
    
    // Test with different tool name (should not detect loop)
    let different_tool_loop_count = agent.check_tool_loop("different_tool", &arguments, 5);
    assert_eq!(different_tool_loop_count, 0);
}

/// Test agent tool execution with real MCP server integration
#[tokio::test]
async fn test_agent_tool_execution_with_real_mcp() {
    use aether::testing::full_integration::{FileServerMcp, connect};
    use rmcp::model::{ClientInfo, Implementation};
    
    // Set up in-memory filesystem
    let filesystem = InMemoryFileSystem::new();
    
    // Create real MCP server with write_file tool
    let server_service = FileServerMcp::new(filesystem.clone());
    let client_info = ClientInfo {
        client_info: Implementation {
            name: "agent-test-client".to_string(),
            version: "0.1.0".to_string(),
        },
        ..Default::default()
    };
    
    // Connect server and client via in-memory transport
    let (_server_handle, mcp_client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");
    
    // Create tool registry and populate it with tools from the connected MCP server
    let mut tool_registry = create_test_tool_registry();
    
    // List tools from the connected MCP server
    let tools_response = mcp_client.list_tools(None).await
        .expect("Failed to list tools from MCP server");
    
    // Register tools in the agent's tool registry
    for rmcp_tool in tools_response.tools {
        tool_registry.register_tool("file_server".to_string(), rmcp_tool);
    }
    
    // Note: The current McpClient doesn't have a way to set a connected client directly
    // This would need to be extended in the real implementation
    // For now, we'll test the agent's tool registry integration
    
    // Create agent with the populated tool registry
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("I'll write the file for you.".to_string()), StreamChunk::Done]);
    let mut agent = Agent::new(llm_provider, tool_registry, None);
    
    // Verify tool is registered correctly in the agent
    let available_tools = agent.tool_registry().list_tools();
    assert!(available_tools.contains(&"write_file".to_string()));
    assert_eq!(agent.get_server_for_tool("write_file"), Some(&"file_server".to_string()));
    
    // Verify tool definition building works
    let tool_definitions = agent.build_tool_definitions();
    assert_eq!(tool_definitions.len(), 1);
    assert_eq!(tool_definitions[0].name, "write_file");
    assert!(tool_definitions[0].description.contains("Write content to a file"));
    
    // Test that the agent can create proper chat requests with tools
    agent.add_message(ChatMessage::User {
        content: "Please write 'Hello Agent' to /tmp/agent.txt".to_string(),
        timestamp: Utc::now(),
    });
    
    let chat_request = agent.create_chat_request(Some(0.7));
    assert_eq!(chat_request.tools.len(), 1);
    assert_eq!(chat_request.tools[0].name, "write_file");
    
    // Test that we can call the tool directly via the connected MCP client
    // (This bypasses the agent but verifies the infrastructure works)
    let tool_call_result = mcp_client.call_tool(rmcp::model::CallToolRequestParam {
        name: "write_file".into(),
        arguments: Some(json!({
            "path": "/tmp/agent_test.txt",
            "content": "Hello from Agent integration test!"
        }).as_object().unwrap().clone()),
    }).await.expect("Failed to call write_file tool via MCP client");
    
    // Verify the tool call was successful
    assert_eq!(tool_call_result.is_error, Some(false));
    assert!(!tool_call_result.content.is_empty());
    
    // Verify the file was actually written to the filesystem
    let file_content = filesystem.read_file("/tmp/agent_test.txt").await
        .expect("File should exist in filesystem");
    assert_eq!(file_content, "Hello from Agent integration test!");
    
    println!("✅ Agent tool registry integration with real MCP server successful!");
    println!("✅ Tool registered in agent's registry");
    println!("✅ Tool definitions built correctly");
    println!("✅ Chat requests include tools");
    println!("✅ MCP server tool execution works");
    println!("✅ File written to in-memory filesystem");
}

/// Test agent's execute_tool method with real MCP tool execution
#[tokio::test]
async fn test_agent_execute_tool_end_to_end() {
    use aether::testing::full_integration::{FileServerMcp, connect};
    use rmcp::model::{ClientInfo, Implementation};
    
    // Set up in-memory filesystem
    let filesystem = InMemoryFileSystem::new();
    
    // Create real MCP server with write_file tool
    let server_service = FileServerMcp::new(filesystem.clone());
    let client_info = ClientInfo {
        client_info: Implementation {
            name: "agent-e2e-test-client".to_string(),
            version: "0.1.0".to_string(),
        },
        ..Default::default()
    };
    
    // Connect server and client via in-memory transport
    let (_server_handle, mcp_service) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");
    
    // Create tool registry and populate it from the MCP server
    let mut tool_registry = create_test_tool_registry();
    
    // List tools from connected server and register them
    let tools_response = mcp_service.list_tools(None).await
        .expect("Failed to list tools from MCP server");
    
    for rmcp_tool in tools_response.tools {
        tool_registry.register_tool("file_server".to_string(), rmcp_tool);
    }
    
    // Create a wrapper MCP client that can use the connected service
    // Note: This is a limitation in the current design - the aether::mcp::McpClient
    // doesn't have a way to wrap an already-connected rmcp client service
    // For demonstration, we'll test what we can with the current architecture
    
    // Create agent 
    let llm_provider = FakeLlmProvider::with_single_response(vec![
        StreamChunk::Content("I'll execute the tool for you.".to_string()),
        StreamChunk::Done
    ]);
    let agent = Agent::new(llm_provider, tool_registry, None);
    
    // Verify the agent has the tools available
    assert!(agent.tool_registry().list_tools().contains(&"write_file".to_string()));
    
    // Test that we can at least attempt tool execution
    // (This will fail because no MCP client is set in the registry, but demonstrates the code path)
    let tool_args = json!({
        "path": "/tmp/agent_execute_test.txt",
        "content": "Hello from agent.execute_tool()!"
    });
    
    let result = agent.execute_tool("write_file", tool_args).await;
    // This should fail with "No MCP client available" error since we haven't set one
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No MCP client available"));
    
    // However, we can directly test tool execution via the MCP service to verify 
    // the infrastructure works end-to-end
    let direct_result = mcp_service.call_tool(rmcp::model::CallToolRequestParam {
        name: "write_file".into(),
        arguments: Some(json!({
            "path": "/tmp/direct_test.txt", 
            "content": "Direct execution test!"
        }).as_object().unwrap().clone()),
    }).await.expect("Direct tool execution should work");
    
    assert_eq!(direct_result.is_error, Some(false));
    
    // Verify the file was written
    let file_content = filesystem.read_file("/tmp/direct_test.txt").await
        .expect("File should exist");
    assert_eq!(file_content, "Direct execution test!");
    
    println!("✅ Agent execute_tool method tested (shows expected error path)");
    println!("✅ Direct MCP tool execution verified working");
    println!("✅ End-to-end tool execution infrastructure confirmed");
    println!("ℹ️  To fully test agent.execute_tool(), the ToolRegistry would need");
    println!("ℹ️  a way to accept an already-connected MCP client service");
}

/// Test agent system prompt handling
#[tokio::test]
async fn test_agent_system_prompt() {
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("Test response".to_string()), StreamChunk::Done]);
    let tool_registry = create_test_tool_registry();
    let system_prompt = Some("You are a specialized coding assistant.".to_string());
    
    let agent = Agent::new(llm_provider, tool_registry, system_prompt);
    
    // Test LLM message building with system prompt
    let llm_messages = agent.build_llm_messages();
    assert_eq!(llm_messages.len(), 1);
    
    if let aether::llm::ChatMessage::System { content } = &llm_messages[0] {
        assert!(content.contains("You are an AI assistant"));
        assert!(content.contains("You are a specialized coding assistant"));
    } else {
        panic!("Expected system message");
    }
}

/// Test agent system prompt with existing system message
#[tokio::test]
async fn test_agent_existing_system_message() {
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("Test response".to_string()), StreamChunk::Done]);
    let tool_registry = create_test_tool_registry();
    let system_prompt = Some("You are a specialized coding assistant.".to_string());
    
    let mut agent = Agent::new(llm_provider, tool_registry, system_prompt);
    
    // Add an explicit system message
    agent.add_message(ChatMessage::System {
        content: "Custom system message".to_string(),
        timestamp: Utc::now(),
    });
    
    // Test LLM message building - should use explicit system message, not the system_prompt
    let llm_messages = agent.build_llm_messages();
    assert_eq!(llm_messages.len(), 1);
    
    if let aether::llm::ChatMessage::System { content } = &llm_messages[0] {
        assert_eq!(content, "Custom system message");
    } else {
        panic!("Expected system message");
    }
}

/// Test agent tool definition building
#[tokio::test]
async fn test_agent_tool_definitions() {
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("Test response".to_string()), StreamChunk::Done]);
    let mut tool_registry = create_test_tool_registry();
    
    // Add some tools to the registry
    let tools = vec![
        ("server1", "tool1", "First tool"),
        ("server1", "tool2", "Second tool"),
        ("server2", "tool3", "Third tool"),
    ];
    
    for (server, name, description) in tools {
        let tool = create_test_rmcp_tool(name, description);
        tool_registry.register_tool(server.to_string(), tool);
    }
    
    let agent = Agent::new(llm_provider, tool_registry, None);
    
    // Test tool definition building
    let tool_definitions = agent.build_tool_definitions();
    assert_eq!(tool_definitions.len(), 3);
    
    let tool_names: Vec<&String> = tool_definitions.iter().map(|t| &t.name).collect();
    assert!(tool_names.contains(&&"tool1".to_string()));
    assert!(tool_names.contains(&&"tool2".to_string()));
    assert!(tool_names.contains(&&"tool3".to_string()));
}

/// Test agent chat request creation
#[tokio::test]
async fn test_agent_chat_request_creation() {
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("Test response".to_string()), StreamChunk::Done]);
    let mut tool_registry = create_test_tool_registry();
    
    // Add a tool
    let tool = create_test_rmcp_tool("test_tool", "A test tool");
    tool_registry.register_tool("test_server".to_string(), tool);
    
    let mut agent = Agent::new(llm_provider, tool_registry, None);
    
    // Add some conversation history
    agent.add_message(ChatMessage::User {
        content: "Hello".to_string(),
        timestamp: Utc::now(),
    });
    agent.add_message(ChatMessage::Assistant {
        content: "Hi there!".to_string(),
        timestamp: Utc::now(),
    });
    
    // Create chat request
    let request = agent.create_chat_request(Some(0.7));
    
    // Verify request structure
    assert_eq!(request.messages.len(), 3); // System + User + Assistant
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.temperature, Some(0.7));
    
    // Check tool is included
    assert_eq!(request.tools[0].name, "test_tool");
    assert_eq!(request.tools[0].description, "A test tool");
}

/// Test agent tool registry updates
#[tokio::test]
async fn test_agent_tool_registry_updates() {
    let llm_provider = FakeLlmProvider::with_single_response(vec![StreamChunk::Content("Test response".to_string()), StreamChunk::Done]);
    let tool_registry = create_test_tool_registry();
    let mut agent = Agent::new(llm_provider, tool_registry, None);
    
    // Initially no tools
    assert_eq!(agent.tool_registry().list_tools().len(), 0);
    
    // Create new registry with tools
    let mut new_registry = create_test_tool_registry();
    let tool = create_test_rmcp_tool("new_tool", "A new tool");
    new_registry.register_tool("new_server".to_string(), tool);
    
    // Update agent's tool registry
    agent.update_tool_registry(new_registry);
    
    // Verify tools are now available
    assert_eq!(agent.tool_registry().list_tools().len(), 1);
    assert!(agent.tool_registry().list_tools().contains(&"new_tool".to_string()));
    assert_eq!(
        agent.get_server_for_tool("new_tool"),
        Some(&"new_server".to_string())
    );
}
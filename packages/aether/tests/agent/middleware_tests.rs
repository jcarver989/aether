use aether::{
    agent::{AgentEvent, AgentMessage, MiddlewareAction, Prompt, UserMessage, agent},
    testing::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
};
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn test_middleware_on_user_message() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();

    let llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "test".to_string(),
        },
        LlmResponse::Text {
            chunk: "Response".to_string(),
        },
        LlmResponse::Done,
    ]);

    let prompt = Prompt::text("test prompt").build().unwrap();
    let mut agent = agent(llm)
        .system(&prompt)
        .on_event(move |event| {
            let captured = captured_clone.clone();
            async move {
                if let AgentEvent::UserMessage { content } = event {
                    captured.lock().unwrap().push(format!("user: {}", content));
                }
                MiddlewareAction::Allow
            }
        })
        .spawn()
        .await
        .unwrap();

    agent.send(UserMessage::text("Hello")).await.unwrap();

    // Wait for response
    while let Ok(Some(msg)) = timeout(Duration::from_secs(2), agent.recv()).await {
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }

    let messages = captured.lock().unwrap().clone();
    assert_eq!(messages, vec!["user: Hello"]);
}

#[tokio::test]
async fn test_middleware_chaining() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured1 = captured.clone();
    let captured2 = captured.clone();

    let llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "test".to_string(),
        },
        LlmResponse::Text {
            chunk: "Response".to_string(),
        },
        LlmResponse::Done,
    ]);

    let prompt = Prompt::text("test prompt").build().unwrap();
    let mut agent = agent(llm)
        .system(&prompt)
        .on_event(move |event| {
            let captured = captured1.clone();
            async move {
                if let AgentEvent::UserMessage { content } = event {
                    captured
                        .lock()
                        .unwrap()
                        .push(format!("handler1: {}", content));
                }
                MiddlewareAction::Allow
            }
        })
        .on_event(move |event| {
            let captured = captured2.clone();
            async move {
                if let AgentEvent::UserMessage { content } = event {
                    captured
                        .lock()
                        .unwrap()
                        .push(format!("handler2: {}", content));
                }
                MiddlewareAction::Allow
            }
        })
        .spawn()
        .await
        .unwrap();

    agent.send(UserMessage::text("Test")).await.unwrap();

    // Wait for response
    while let Ok(Some(msg)) = timeout(Duration::from_secs(2), agent.recv()).await {
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }

    let messages = captured.lock().unwrap().clone();
    assert_eq!(messages[0], "handler1: Test");
    assert_eq!(messages[1], "handler2: Test");
}

#[tokio::test]
async fn test_middleware_block_user_message() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();

    let llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "test".to_string(),
        },
        LlmResponse::Text {
            chunk: "Response".to_string(),
        },
        LlmResponse::Done,
    ]);

    let prompt = Prompt::text("test prompt").build().unwrap();
    let mut agent = agent(llm)
        .system(&prompt)
        .on_event(move |event| {
            let captured = captured_clone.clone();
            async move {
                match event {
                    AgentEvent::UserMessage { content } => {
                        captured.lock().unwrap().push(format!("blocked: {}", content));
                        // Block all user messages
                        return MiddlewareAction::Block;
                    }
                    AgentEvent::ToolCall { .. } => {
                        captured.lock().unwrap().push("tool".to_string());
                    }
                }
                MiddlewareAction::Allow
            }
        })
        .spawn()
        .await
        .unwrap();

    agent.send(UserMessage::text("Test")).await.unwrap();

    // Should receive an error message
    let msg = timeout(Duration::from_secs(2), agent.recv())
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(msg, AgentMessage::Error { message } if message == "Message blocked by middleware"));

    let messages = captured.lock().unwrap().clone();
    // The user message should have been seen by middleware
    assert_eq!(messages, vec!["blocked: Test"]);
}

#[tokio::test]
async fn test_middleware_blocks_tool_call() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();

    let llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "test".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "tool1".to_string(),
            name: "dangerous_tool".to_string(),
        },
        LlmResponse::ToolRequestArg {
            id: "tool1".to_string(),
            chunk: "{}".to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool1".to_string(),
                name: "dangerous_tool".to_string(),
                arguments: "{}".to_string(),
            },
        },
        LlmResponse::Done,
    ]);

    let prompt = Prompt::text("test prompt").build().unwrap();
    let mut agent = agent(llm)
        .system(&prompt)
        .on_event(move |event| {
            let captured = captured_clone.clone();
            async move {
                match event {
                    AgentEvent::ToolCall { name, .. } => {
                        captured.lock().unwrap().push(format!("tool: {}", name));
                        if name == "dangerous_tool" {
                            return MiddlewareAction::Block;
                        }
                    }
                    _ => {}
                }
                MiddlewareAction::Allow
            }
        })
        .spawn()
        .await
        .unwrap();

    agent.send(UserMessage::text("Test")).await.unwrap();

    let mut error_received = false;
    // Wait for response
    while let Ok(Some(msg)) = timeout(Duration::from_secs(2), agent.recv()).await {
        match msg {
            AgentMessage::Error { message } => {
                assert_eq!(message, "Tool 'dangerous_tool' blocked by middleware");
                error_received = true;
            }
            AgentMessage::Done => break,
            _ => {}
        }
    }

    assert!(error_received, "Expected error message when tool was blocked");

    let messages = captured.lock().unwrap().clone();
    assert_eq!(messages, vec!["tool: dangerous_tool"]);
}

#[tokio::test]
async fn test_middleware_any_block_wins() {
    let llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "test".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "tool1".to_string(),
            name: "test_tool".to_string(),
        },
        LlmResponse::ToolRequestArg {
            id: "tool1".to_string(),
            chunk: "{}".to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool1".to_string(),
                name: "test_tool".to_string(),
                arguments: "{}".to_string(),
            },
        },
        LlmResponse::Done,
    ]);

    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured1 = captured.clone();
    let captured2 = captured.clone();
    let captured3 = captured.clone();

    let prompt = Prompt::text("test prompt").build().unwrap();
    let mut agent = agent(llm)
        .system(&prompt)
        .on_event(move |event| {
            let captured = captured1.clone();
            async move {
                if let AgentEvent::ToolCall { .. } = event {
                    captured.lock().unwrap().push("handler1: allow".to_string());
                    return MiddlewareAction::Allow;
                }
                MiddlewareAction::Allow
            }
        })
        .on_event(move |event| {
            let captured = captured2.clone();
            async move {
                if let AgentEvent::ToolCall { .. } = event {
                    captured.lock().unwrap().push("handler2: block".to_string());
                    return MiddlewareAction::Block;
                }
                MiddlewareAction::Allow
            }
        })
        .on_event(move |event| {
            let captured = captured3.clone();
            async move {
                if let AgentEvent::ToolCall { .. } = event {
                    captured.lock().unwrap().push("handler3: allow".to_string());
                    return MiddlewareAction::Allow;
                }
                MiddlewareAction::Allow
            }
        })
        .spawn()
        .await
        .unwrap();

    agent.send(UserMessage::text("Test")).await.unwrap();

    // Wait for response
    while let Ok(Some(msg)) = timeout(Duration::from_secs(2), agent.recv()).await {
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }

    let messages = captured.lock().unwrap().clone();
    // All three handlers should have run
    assert_eq!(messages.len(), 3);
    assert!(messages.contains(&"handler1: allow".to_string()));
    assert!(messages.contains(&"handler2: block".to_string()));
    assert!(messages.contains(&"handler3: allow".to_string()));
    // But the tool should have been blocked (we can verify this by checking
    // that no ToolResult event was emitted - which we'd need to test separately)
}

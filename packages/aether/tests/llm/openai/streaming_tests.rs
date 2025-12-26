use async_openai::types::chat::{
    ChatChoiceStream, ChatCompletionMessageToolCallChunk, ChatCompletionStreamResponseDelta,
    CreateChatCompletionStreamResponse, FinishReason, FunctionCallStream,
};
use tokio_stream::StreamExt;

use aether::llm::LlmResponse;
use aether::llm::openai::streaming::process_completion_stream;

#[tokio::test]
async fn test_parallel_tool_calls() {
    // Create a stream with parallel tool calls at different indices
    let stream_items = vec![
        // Start of first tool call (index 0)
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: Some(vec![ChatCompletionMessageToolCallChunk {
                            index: 0,
                            id: Some("call_1".to_string()),
                            r#type: None,
                            function: Some(FunctionCallStream {
                                name: Some("function_a".to_string()),
                                arguments: None,
                            }),
                        }]),
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // Start of second tool call (index 1)
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: Some(vec![ChatCompletionMessageToolCallChunk {
                            index: 1,
                            id: Some("call_2".to_string()),
                            r#type: None,
                            function: Some(FunctionCallStream {
                                name: Some("function_b".to_string()),
                                arguments: None,
                            }),
                        }]),
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // Arguments for first tool call (index 0)
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: Some(vec![ChatCompletionMessageToolCallChunk {
                            index: 0,
                            id: None,
                            r#type: None,
                            function: Some(FunctionCallStream {
                                name: None,
                                arguments: Some("{\"param\":".to_string()),
                            }),
                        }]),
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // Arguments for second tool call (index 1)
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: Some(vec![ChatCompletionMessageToolCallChunk {
                            index: 1,
                            id: None,
                            r#type: None,
                            function: Some(FunctionCallStream {
                                name: None,
                                arguments: Some("{\"value\":".to_string()),
                            }),
                        }]),
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // More arguments for first tool call (index 0)
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: Some(vec![ChatCompletionMessageToolCallChunk {
                            index: 0,
                            id: None,
                            r#type: None,
                            function: Some(FunctionCallStream {
                                name: None,
                                arguments: Some("\"test\"}".to_string()),
                            }),
                        }]),
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // More arguments for second tool call (index 1)
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: Some(vec![ChatCompletionMessageToolCallChunk {
                            index: 1,
                            id: None,
                            r#type: None,
                            function: Some(FunctionCallStream {
                                name: None,
                                arguments: Some("42}".to_string()),
                            }),
                        }]),
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // Finish the stream
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: None,
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: Some(FinishReason::ToolCalls),
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
    ];

    let stream = tokio_stream::iter(stream_items);
    let mut processed_stream = Box::pin(process_completion_stream(stream));

    let mut events = Vec::new();
    while let Some(event) = processed_stream.next().await {
        events.push(event.unwrap());
    }

    // Verify the stream structure
    assert!(matches!(events[0], LlmResponse::Start { .. }));

    // Should have two tool request starts
    let mut tool_starts = 0;
    let mut tool_args = 0;
    let mut tool_completions = 0;

    for event in &events {
        match event {
            LlmResponse::ToolRequestStart { id, name } => {
                tool_starts += 1;
                assert!(id == "call_1" || id == "call_2");
                assert!(name == "function_a" || name == "function_b");
            }
            LlmResponse::ToolRequestArg { id, chunk: _ } => {
                tool_args += 1;
                assert!(id == "call_1" || id == "call_2");
            }
            LlmResponse::ToolRequestComplete { tool_call } => {
                tool_completions += 1;
                if tool_call.id == "call_1" {
                    assert_eq!(tool_call.name, "function_a");
                    assert_eq!(tool_call.arguments, "{\"param\":\"test\"}");
                } else if tool_call.id == "call_2" {
                    assert_eq!(tool_call.name, "function_b");
                    assert_eq!(tool_call.arguments, "{\"value\":42}");
                } else {
                    panic!("Unexpected tool call id: {}", tool_call.id);
                }
            }
            _ => {}
        }
    }

    assert_eq!(tool_starts, 2, "Should have 2 tool request starts");
    assert_eq!(tool_args, 4, "Should have 4 tool argument chunks");
    assert_eq!(tool_completions, 2, "Should have 2 tool completions");

    assert!(matches!(events.last(), Some(LlmResponse::Done)));
}

#[tokio::test]
async fn test_tool_call_followed_by_content() {
    // Test that when tool call is followed by content, the tool call is completed first
    let stream_items = vec![
        // Tool call start
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: Some(vec![ChatCompletionMessageToolCallChunk {
                            index: 0,
                            id: Some("call_1".to_string()),
                            r#type: None,
                            function: Some(FunctionCallStream {
                                name: Some("test_func".to_string()),
                                arguments: None,
                            }),
                        }]),
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // Tool call arguments
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: Some(vec![ChatCompletionMessageToolCallChunk {
                            index: 0,
                            id: None,
                            r#type: None,
                            function: Some(FunctionCallStream {
                                name: None,
                                arguments: Some("{}".to_string()),
                            }),
                        }]),
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // Content after tool call
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: Some("Here is the result".to_string()),
                        tool_calls: None,
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
        // Finish
        Ok::<CreateChatCompletionStreamResponse, std::io::Error>(
            CreateChatCompletionStreamResponse {
                choices: vec![ChatChoiceStream {
                    delta: ChatCompletionStreamResponseDelta {
                        content: None,
                        tool_calls: None,
                        role: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: Some(FinishReason::Stop),
                    index: 0,
                    logprobs: None,
                }],
                id: "test".to_string(),
                created: 0,
                model: "test".to_string(),
                object: "chat.completion.chunk".to_string(),
                system_fingerprint: None,
                usage: None,
                service_tier: None,
            },
        ),
    ];

    let stream = tokio_stream::iter(stream_items);
    let mut processed_stream = Box::pin(process_completion_stream(stream));

    let mut events = Vec::new();
    while let Some(event) = processed_stream.next().await {
        events.push(event.unwrap());
    }

    // Find the tool completion and text events
    let mut tool_completion_index = None;
    let mut text_index = None;

    for (i, event) in events.iter().enumerate() {
        match event {
            LlmResponse::ToolRequestComplete { .. } => {
                tool_completion_index = Some(i);
            }
            LlmResponse::Text { .. } => {
                text_index = Some(i);
            }
            _ => {}
        }
    }

    // Tool completion should come before text
    assert!(tool_completion_index.is_some());
    assert!(text_index.is_some());
    assert!(
        tool_completion_index.unwrap() < text_index.unwrap(),
        "Tool completion should come before text content"
    );
}

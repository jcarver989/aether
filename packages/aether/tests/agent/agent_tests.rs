use std::error::Error;
use std::time::Duration;

use aether::{
    core::COMPLETION_SIGNAL,
    events::{AgentMessage, UserMessage},
    testing::{
        agent_message, test_agent,
        {AddNumbersRequest, AddNumbersResult, DivideNumbersRequest, SlowToolRequest},
    },
};
use llm::ChatMessage;
use llm::testing::llm_response;

#[tokio::test]
async fn test_text_message() -> Result<(), Box<dyn Error>> {
    let id = "message_1";
    let chunks = ["Hello", "user", COMPLETION_SIGNAL];
    let llm_responses = [llm_response(id).text(&chunks).build()];
    let mut expected_messages = agent_message(id).text(&chunks).build();
    expected_messages.push(AgentMessage::Done);

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("hi")])
        .run()
        .await?;
    assert_eq!(messages, expected_messages);
    Ok(())
}

#[tokio::test]
async fn test_single_tool_call() -> Result<(), Box<dyn Error>> {
    let tool_request = AddNumbersRequest::new(3, 5);
    let tool_result = AddNumbersResult::new(8);
    let (m1_id, t1_id, t1_name) = ("message_1", "call_1", "test__add_numbers");
    let m2_id = "message-2";
    let chunks = ["The", " sum", " is", " 8", COMPLETION_SIGNAL];

    let llm_responses = [
        llm_response(m1_id)
            .tool_call(t1_id, t1_name, &[&tool_request.json()?])
            .build(),
        llm_response(m2_id).text(&chunks).build(),
    ];

    let expected_messages = {
        let mut messages = Vec::new();
        messages.extend(
            agent_message(m1_id)
                .tool_call(t1_id, t1_name, &tool_request, &tool_result)
                .build(),
        );

        messages.extend(agent_message(m2_id).text(&chunks).build());
        messages.push(AgentMessage::Done);
        messages
    };

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("3+5 = ?")])
        .run()
        .await?;
    assert_eq!(messages, expected_messages);
    Ok(())
}

#[tokio::test]
async fn test_tool_call_failure() -> Result<(), Box<dyn Error>> {
    let tool_request = DivideNumbersRequest::new(10, 0);
    let chunks = [
        "I",
        " apologize",
        ",",
        " but",
        " division",
        " by",
        " zero",
        " is",
        " not",
        " allowed",
        ".",
        COMPLETION_SIGNAL,
    ];

    let llm_responses = [
        llm_response("message_1")
            .tool_call("call_1", "test__divide_numbers", &[&tool_request.json()?])
            .build(),
        llm_response("message_2").text(&chunks).build(),
    ];

    let expected_messages = {
        let mut messages = Vec::new();
        messages.extend(
            agent_message("message_1")
                .tool_call_with_error(
                    "call_1",
                    "test__divide_numbers",
                    &tool_request,
                    "Division by zero",
                )
                .build(),
        );

        messages.extend(agent_message("message_2").text(&chunks).build());
        messages.push(AgentMessage::Done);
        messages
    };

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("10 / 0 = ?")])
        .run()
        .await?;
    assert_eq!(messages, expected_messages);
    Ok(())
}

#[tokio::test]
async fn test_cancellation() -> Result<(), Box<dyn Error>> {
    let chunks = [
        "This",
        " is",
        " a",
        " long",
        " response",
        " to",
        " ensure",
        " cancellation",
        " happens",
        " during",
        " processing",
    ];

    let llm_responses = [llm_response("message_1").text(&chunks).build()];
    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("hi"), UserMessage::Cancel])
        .run()
        .await?;

    let text_chunks_received = messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::Text { .. }))
        .count();

    assert!(
        messages
            .iter()
            .any(|m| matches!(m, AgentMessage::Cancelled { .. })),
        "Expected to receive a Cancelled message"
    );

    // Due to Agent's merging of N async streams, it's hard to control
    // exact ordering, so we use a coarse grained aseertion here
    assert!(
        text_chunks_received < chunks.len(),
        "Expected cancellation to stop processing before all {} chunks were sent, but received {}",
        chunks.len(),
        text_chunks_received
    );

    Ok(())
}

#[tokio::test]
async fn test_tool_timeout() -> Result<(), Box<dyn Error>> {
    let tool_duration = 2000;
    let tool_timeout = 500;

    let tool_request = SlowToolRequest::new(tool_duration);
    let (m1_id, t1_id, t1_name) = ("message_1", "call_1", "test__slow_tool");

    let llm_responses = [llm_response(m1_id)
        .tool_call(t1_id, t1_name, &[&tool_request.json()?])
        .build()];

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("run slow tool")])
        .tool_timeout(Duration::from_millis(tool_timeout))
        .run()
        .await?;

    let has_tool_error = messages.iter().any(|m| {
        matches!(
            m,
            AgentMessage::ToolError { error, .. } if error.error.contains("timeout")
        )
    });

    assert!(
        has_tool_error,
        "Expected a ToolError with timeout message, got: {messages:?}"
    );

    Ok(())
}

#[tokio::test]
async fn test_simple_message_content() -> Result<(), Box<dyn Error>> {
    let (id, chunks) = ("message_1", ["Hello"]);
    let llm_responses = [llm_response(id).text(&chunks).build()];

    let result = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("Just a simple message")])
        .run_with_context()
        .await?;

    let contexts = result.captured_contexts.lock().unwrap();
    let first_context = &contexts[0];
    let messages = first_context.messages();

    let user_message = messages
        .iter()
        .find(|m| matches!(m, ChatMessage::User { .. }))
        .expect("Expected a user message");

    let content = match user_message {
        ChatMessage::User { content, .. } => content,
        _ => panic!("Expected User message"),
    };

    // Content should be exactly the user's message
    assert_eq!(content, "Just a simple message");

    Ok(())
}

#[tokio::test]
async fn test_auto_continue_stops_with_completion_signal() -> Result<(), Box<dyn Error>> {
    let chunks = ["I have completed the task. ", COMPLETION_SIGNAL];
    let llm_responses = [llm_response("msg_1").text(&chunks).build()];

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("do something")])
        .max_auto_continues(3)
        .run()
        .await?;

    let auto_continue_count = messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::AutoContinue { .. }))
        .count();
    assert_eq!(
        auto_continue_count, 0,
        "Expected no AutoContinue messages when completion signal is present"
    );

    assert!(
        matches!(messages.last(), Some(AgentMessage::Done)),
        "Expected Done message"
    );

    Ok(())
}

#[tokio::test]
async fn test_auto_continue_not_triggered_for_opening_message() -> Result<(), Box<dyn Error>> {
    let chunks = ["Hey there!", " How can I help?"];

    let llm_responses = [llm_response("msg_1").text(&chunks).build()];

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("hello")])
        .max_auto_continues(3)
        .run()
        .await?;

    let auto_continue_count = messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::AutoContinue { .. }))
        .count();
    assert_eq!(
        auto_continue_count, 0,
        "Expected no AutoContinue messages for opening message without tool calls"
    );

    assert!(
        matches!(messages.last(), Some(AgentMessage::Done)),
        "Expected Done message for opening message"
    );

    Ok(())
}

#[tokio::test]
async fn test_auto_continue_triggers_without_completion_signal() -> Result<(), Box<dyn Error>> {
    let tool_request = AddNumbersRequest::new(2, 3);
    let final_chunks = ["Done! ", COMPLETION_SIGNAL];

    let llm_responses = [
        llm_response("msg_1")
            .tool_call("call_1", "test__add_numbers", &[&tool_request.json()?])
            .build(),
        llm_response("msg_2")
            .text(&["I'm thinking about the problem..."])
            .build(),
        llm_response("msg_3").text(&["Let me continue..."]).build(),
        llm_response("msg_4").text(&final_chunks).build(),
    ];

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("do something")])
        .max_auto_continues(5)
        .run()
        .await?;

    let auto_continue_count = messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::AutoContinue { .. }))
        .count();
    assert_eq!(
        auto_continue_count, 2,
        "Expected 2 AutoContinue messages after tool call, got {}",
        auto_continue_count
    );

    let auto_continues: Vec<_> = messages
        .iter()
        .filter_map(|m| match m {
            AgentMessage::AutoContinue {
                attempt,
                max_attempts,
            } => Some((*attempt, *max_attempts)),
            _ => None,
        })
        .collect();
    assert_eq!(auto_continues, vec![(1, 5), (2, 5)]);

    Ok(())
}

#[tokio::test]
async fn test_auto_continue_respects_max_limit() -> Result<(), Box<dyn Error>> {
    let tool_request = AddNumbersRequest::new(2, 3);

    let llm_responses = [
        llm_response("msg_1")
            .tool_call("call_1", "test__add_numbers", &[&tool_request.json()?])
            .build(),
        llm_response("msg_2").text(&["Thinking..."]).build(),
        llm_response("msg_3").text(&["Still thinking..."]).build(),
        llm_response("msg_4").text(&["More thinking..."]).build(),
        llm_response("msg_5")
            .text(&["This should not appear"])
            .build(),
    ];

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("do something")])
        .max_auto_continues(2)
        .run()
        .await?;

    let auto_continue_count = messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::AutoContinue { .. }))
        .count();
    assert_eq!(
        auto_continue_count, 2,
        "Expected 2 AutoContinue messages (max limit), got {}",
        auto_continue_count
    );

    assert!(
        matches!(messages.last(), Some(AgentMessage::Done)),
        "Expected Done message after hitting max_auto_continues"
    );

    Ok(())
}

#[tokio::test]
async fn test_auto_continue_disabled_with_zero() -> Result<(), Box<dyn Error>> {
    let tool_request = AddNumbersRequest::new(2, 3);

    let llm_responses = [
        llm_response("msg_1")
            .tool_call("call_1", "test__add_numbers", &[&tool_request.json()?])
            .build(),
        llm_response("msg_2")
            .text(&["No completion signal here"])
            .build(),
    ];

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("do something")])
        .max_auto_continues(0)
        .run()
        .await?;

    let auto_continue_count = messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::AutoContinue { .. }))
        .count();
    assert_eq!(
        auto_continue_count, 0,
        "Expected no AutoContinue messages when max_auto_continues=0"
    );

    assert!(
        matches!(messages.last(), Some(AgentMessage::Done)),
        "Expected Done message"
    );

    Ok(())
}

#[tokio::test]
async fn test_reasoning_content_is_saved_in_context_after_tool_call() -> Result<(), Box<dyn Error>>
{
    use llm::LlmResponse;

    let tool_request = AddNumbersRequest::new(2, 3);

    let llm_responses = [
        vec![
            LlmResponse::start("msg_1"),
            LlmResponse::reasoning("internal plan"),
            LlmResponse::tool_request_start("call_1", "test__add_numbers"),
            LlmResponse::tool_request_arg("call_1", &tool_request.json()?),
            LlmResponse::tool_request_complete(
                "call_1",
                "test__add_numbers",
                &tool_request.json()?,
            ),
            LlmResponse::Done,
        ],
        llm_response("msg_2")
            .text(&["Done ", COMPLETION_SIGNAL])
            .build(),
    ];

    let result = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(vec![UserMessage::text("do something")])
        .run_with_context()
        .await?;

    let contexts = result.captured_contexts.lock().unwrap();
    let second_context = contexts
        .get(1)
        .expect("expected second LLM request context");

    let assistant_with_tool_call = second_context.messages().iter().find(|message| {
        matches!(
            message,
            ChatMessage::Assistant { tool_calls, .. } if !tool_calls.is_empty()
        )
    });

    let Some(ChatMessage::Assistant {
        reasoning_content, ..
    }) = assistant_with_tool_call
    else {
        panic!("expected assistant message with tool call");
    };

    assert_eq!(reasoning_content.as_deref(), Some("internal plan"));

    Ok(())
}

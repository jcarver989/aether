use std::error::Error;
use std::time::Duration;

use aether::{
    agent::{AgentMessage, UserMessage},
    testing::{
        agent_message, llm_response, test_agent,
        {AddNumbersRequest, AddNumbersResult, DivideNumbersRequest, SlowToolRequest},
    },
};

#[tokio::test]
async fn test_text_message() -> Result<(), Box<dyn Error>> {
    let (id, chunks) = ("message_1", ["Hello", "user"]);
    let llm_responses = [llm_response(id).text(&chunks).build()];
    let mut expected_messages = agent_message(id).text(&chunks).build();
    expected_messages.push(AgentMessage::Done);

    let messages = test_agent()
        .llm_responses(&llm_responses)
        .user_messages(&[UserMessage::text("hi")])
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
    let (m2_id, chunks) = ("message-2", ["The", " sum", " is", " 8"]);

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
        .user_messages(&[UserMessage::text("3+5 = ?")])
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
        .user_messages(&[UserMessage::text("10 / 0 = ?")])
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
        .user_messages(&[UserMessage::text("hi"), UserMessage::Cancel])
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
        .user_messages(&[UserMessage::text("run slow tool")])
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

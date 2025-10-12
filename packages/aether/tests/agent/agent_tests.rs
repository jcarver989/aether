use std::error::Error;

use aether::{
    agent::{AgentMessage, UserMessage},
    testing::{
        agent_message::agent_message,
        collect_agent_messages,
        fake_mcp::{AddNumbersRequest, AddNumbersResult, DivideNumbersRequest},
        llm_response::llm_response,
        utils::run_agent,
    },
};

#[tokio::test]
async fn test_text_message() -> Result<(), Box<dyn Error>> {
    let llm_responses = [llm_response("message_1").text(&["Hello", "user"]).build()];
    let mut expected_messages = agent_message("message_1").text(&["Hello", "user"]).build();
    expected_messages.push(AgentMessage::Done);
    run_agent(
        &llm_responses,
        &[UserMessage::text("hi")],
        expected_messages,
    )
    .await
}

#[tokio::test]
async fn test_single_tool_call() -> Result<(), Box<dyn Error>> {
    let tool_request = AddNumbersRequest::new(3, 5);
    let tool_result = AddNumbersResult::new(8);

    let llm_responses = [
        llm_response("message_1")
            .tool_call("call_1", "test__add_numbers", &[&tool_request.json()?])
            .build(),
        llm_response("message_2")
            .text(&["The", " sum", " is", " 8"])
            .build(),
    ];

    let expected_messages = {
        let mut messages = Vec::new();
        messages.extend(
            agent_message("message_1")
                .tool_call("call_1", "test__add_numbers", &tool_request, &tool_result)
                .build(),
        );

        messages.extend(
            agent_message("message_2")
                .text(&["The", " sum", " is", " 8"])
                .build(),
        );

        messages.push(AgentMessage::Done);
        messages
    };

    run_agent(
        &llm_responses,
        &[UserMessage::text("3+5 = ?")],
        expected_messages,
    )
    .await
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

    run_agent(
        &llm_responses,
        &[UserMessage::text("10 / 0 = ?")],
        expected_messages,
    )
    .await
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
    let messages = collect_agent_messages(
        &llm_responses,
        &[UserMessage::text("hi"), UserMessage::Cancel],
    )
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

use std::error::Error;

use aether::{
    agent::{AgentMessage, UserMessage},
    testing::{
        agent_message::agent_message,
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
    let llm_responses = [
        llm_response("message_1")
            .tool_call("call_1", "test__divide_numbers", &[&tool_request.json()?])
            .build(),
        llm_response("message_2")
            .text(&[
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
            ])
            .build(),
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

        messages.extend(
            agent_message("message_2")
                .text(&[
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
                ])
                .build(),
        );

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
    // With the new streaming architecture, Cancel messages are processed immediately
    // in the merged stream. When sent concurrently with a text message, the cancel
    // may arrive before any LLM chunks are emitted.
    let llm_responses = [llm_response("message_1")
        .text(&[
            "This", " is", " a", " longer", " response", " with", " many", " chunks", " to",
            " ensure", " cancellation", " happens", " during", " processing",
        ])
        .build()];

    let expected_messages = vec![AgentMessage::Cancelled {
        message: "Processing cancelled".to_string(),
    }];

    run_agent(
        &llm_responses,
        &[UserMessage::text("hi"), UserMessage::Cancel],
        expected_messages,
    )
    .await
}

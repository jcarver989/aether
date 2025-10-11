use std::error::Error;

use aether::{
    agent::{AgentMessage, UserMessage, agent},
    testing::{
        FakeLlmProvider, FakeMcpServer,
        agent_message::agent_message,
        fake_mcp::{AddNumbersRequest, AddNumbersResult, DivideNumbersRequest, fake_mcp},
        llm_response::llm_response,
    },
    types::LlmResponse,
};

#[tokio::test]
async fn test_text_message() -> Result<(), Box<dyn Error>> {
    let llm_responses = [llm_response("message_1").text(&["Hello", "user"]).build()];
    let mut expected_messages = agent_message("message_1").text(&["Hello", "user"]).build();
    expected_messages.push(AgentMessage::Done);
    run_agent(&llm_responses, UserMessage::text("hi"), expected_messages).await
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
        UserMessage::text("3+5 = ?"),
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
                    "Annotated { raw: Text(RawTextContent { text: \"Division by zero\", meta: None }), annotations: None }",
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
        UserMessage::text("10 / 0 = ?"),
        expected_messages,
    )
    .await
}

async fn run_agent(
    llm_responses: &[Vec<LlmResponse>],
    user_message: UserMessage,
    expected_agent_messages: Vec<AgentMessage>,
) -> Result<(), Box<dyn Error>> {
    let llm = FakeLlmProvider::new(Vec::from(llm_responses));

    let mut handle = agent(llm)
        .mcp(fake_mcp("test", FakeMcpServer::new()))
        .spawn()
        .await?;

    let _ = handle.send(user_message.clone()).await?;
    let mut messages = Vec::new();

    while let Some(message) = handle.recv().await {
        match message {
            AgentMessage::Done => {
                messages.push(AgentMessage::Done);
                break;
            }

            _ => {
                messages.push(message);
            }
        }
    }

    assert_eq!(messages, expected_agent_messages);
    Ok(())
}

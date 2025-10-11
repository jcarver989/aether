use std::error::Error;

use aether::{
    agent::{AgentHandle, AgentMessage, UserMessage, agent},
    testing::{
        FakeMcpServer,
        agent_message::agent_message,
        fake_llm::fake_llm,
        fake_mcp::{AddNumbersRequest, AddNumbersResult, fake_mcp},
        llm_response::llm_response,
    },
};

#[tokio::test]
async fn test_text_message() -> Result<(), Box<dyn Error>> {
    let llm = {
        let message = llm_response("message_1").text(&["Hello", "user"]).build();
        fake_llm(&[message])
    };

    let handle = agent(llm).spawn().await?;
    let agent_messages = run_agent(handle, UserMessage::text("hello")).await?;
    let mut expected_messages = agent_message("message_1").text(&["Hello", "user"]).build();
    expected_messages.push(AgentMessage::Done);
    assert_eq!(agent_messages, expected_messages);
    Ok(())
}

#[tokio::test]
async fn test_single_tool_call() -> Result<(), Box<dyn Error>> {
    let mcp = fake_mcp("test", FakeMcpServer::new());
    let tool_request = AddNumbersRequest::new(3, 5);
    let tool_result = AddNumbersResult::new(8);

    let llm = {
        let first_response = llm_response("message_1")
            .tool_call(
                "call_1",
                &format!("{}__add_numbers", mcp.name()),
                &[&tool_request.json()?],
            )
            .build();

        let second_response = llm_response("message_2")
            .text(&["The", " sum", " is", " 8"])
            .build();

        fake_llm(&[first_response, second_response])
    };

    let handle = agent(llm).mcp(mcp).spawn().await?;
    let agent_messages = run_agent(handle, UserMessage::text("3+5 = ?")).await?;

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
    assert_eq!(agent_messages, expected_messages);
    Ok(())
}

async fn run_agent(
    mut handle: AgentHandle,
    user_message: UserMessage,
) -> Result<Vec<AgentMessage>, Box<dyn Error>> {
    let _ = handle.send(user_message).await?;

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

    Ok(messages)
}

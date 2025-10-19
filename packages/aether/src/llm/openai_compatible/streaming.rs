use crate::llm::openai::process_completion_stream;
use crate::llm::openai_compatible::types::ChatCompletionStreamResponse;
use crate::llm::{LlmError, LlmResponseStream};
use async_openai::{Client, config::OpenAIConfig, types::CreateChatCompletionRequest};
use async_stream;
use tokio_stream::StreamExt;

/// Creates a streaming response for OpenAI-compatible APIs.
/// This allows providers like OpenRouter and Z.ai to reuse the same streaming logic
/// while handling their API quirks through unified types.
pub fn create_custom_stream(
    client: &Client<OpenAIConfig>,
    request: CreateChatCompletionRequest,
) -> LlmResponseStream {
    let client = client.clone();

    Box::pin(async_stream::stream! {
        // Create the stream - need await so we must use async_stream
        let stream = match client
            .chat()
            .create_stream_byot::<CreateChatCompletionRequest, ChatCompletionStreamResponse>(request)
            .await {
            Ok(stream) => stream,
            Err(e) => {
                yield Err(LlmError::ApiRequest(e.to_string()));
                return;
            }
        };

        // Map to standard opneai types
        let mapped_stream = stream.map(|result| {
            result
                .map(|response| response.into())
                .map_err(|e| LlmError::ApiError(e.to_string()))
        });

        for await item in process_completion_stream(mapped_stream) {
            yield item;
        }
    })
}

//! Common types and streaming logic for API providers that are
//! mostly compatible with the OpenAI API but have minor deviations such as:
//! - Missing or optional fields
//! - Different field types (e.g., i64 vs u32 for token counts)
//! - Additional enum variants
//!
//! Providers like OpenRouter, Z.ai, and others can use these utilities to avoid code duplication.

pub mod streaming;
pub mod types;

use async_openai::types::CreateChatCompletionRequest;

use crate::llm::Context;
use crate::llm::openai::mappers::{map_messages, map_tools};

pub use streaming::create_custom_stream;
pub use types::ChatCompletionStreamResponse;

/// Build a chat completion request from a context
///
/// This is shared logic for OpenAI-compatible providers like OpenRouter and Z.ai.
pub fn build_chat_request(model: &str, context: &Context) -> CreateChatCompletionRequest {
    let messages = map_messages(context.messages());
    let tools = if context.tools().is_empty() {
        None
    } else {
        Some(map_tools(context.tools()))
    };

    CreateChatCompletionRequest {
        model: model.to_string(),
        messages,
        stream: Some(true),
        tools,
        ..Default::default()
    }
}

//! Common types and streaming logic for API providers that are
//! mostly compatible with the `OpenAI` API but have minor deviations such as:
//! - Missing or optional fields
//! - Different field types (e.g., i64 vs u32 for token counts)
//! - Additional enum variants
//!
//! Providers like `OpenRouter`, Z.ai, and others can use these utilities to avoid code duplication.

pub mod streaming;
pub mod types;

use crate::providers::openai::mappers::map_tools;
use crate::{Context, LlmError};

pub use streaming::create_custom_stream_generic;
pub use types::{ChatCompletionStreamResponse, CompatibleChatRequest};

/// Build a chat completion request from a context
///
/// This is shared logic for OpenAI-compatible providers like `OpenRouter` and Z.ai.
/// Uses `CompatibleChatRequest` which preserves `reasoning_content` on assistant messages.
pub fn build_chat_request(
    model: &str,
    context: &Context,
) -> Result<CompatibleChatRequest, LlmError> {
    let messages = types::map_messages(context.messages());
    let tools = if context.tools().is_empty() {
        None
    } else {
        Some(map_tools(context.tools())?)
    };

    Ok(CompatibleChatRequest {
        model: model.to_string(),
        messages,
        stream: Some(true),
        tools,
    })
}

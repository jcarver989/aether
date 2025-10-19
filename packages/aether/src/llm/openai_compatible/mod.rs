//! Common types and streaming logic for API providers that are
//! mostly compatible with the OpenAI API but have minor deviations such as:
//! - Missing or optional fields
//! - Different field types (e.g., i64 vs u32 for token counts)
//! - Additional enum variants
//!
//! Providers like OpenRouter, Z.ai, and others can use these utilities to avoid code duplication.

pub mod streaming;
pub mod types;

pub use streaming::create_custom_stream;
pub use types::ChatCompletionStreamResponse;

pub mod alloyed;
pub mod anthropic;
pub mod local;
pub mod openai;
pub mod openrouter;
pub mod provider;

pub use provider::{Context, LlmResponseStream, ModelProvider};

pub enum LlmProviderConfig {
    Anthropic { model: String },
    OpenAI { model: String },
    OpenRouter { model: String },

    Ollama { model: String },
    LlamaCpp,
}

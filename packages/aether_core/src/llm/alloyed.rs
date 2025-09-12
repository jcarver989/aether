use crate::llm::{ModelProvider, local::LocalModelProvider, openrouter::OpenRouterProvider};

pub enum AlloyedModelProviderEnum {
    Local(LocalModelProvider),
    OpenRouter(OpenRouterProvider),
}

pub struct AlloyedModelProvider {
    providers: Vec<AlloyedModelProviderEnum>,
}

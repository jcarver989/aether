use std::path::PathBuf;

use async_trait::async_trait;
use crate::AgentRunnerMessage;

pub struct HookInput {
    pub working_directory: PathBuf,
    pub messages: Vec<AgentRunnerMessage>,
}

pub type HookResult = Result<(), Box<dyn std::error::Error>>;

/// Trait for eval lifecycle hooks (useful for running setup functions)
#[async_trait]
pub trait Hook: Send + Sync {
    async fn run(&self, input: HookInput) -> HookResult;
}

// Implement for closures that return futures
#[async_trait]
impl<F, Fut> Hook for F
where
    F: Fn(HookInput) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = HookResult> + Send + 'static,
{
    async fn run(&self, input: HookInput) -> HookResult {
        self(input).await
    }
}

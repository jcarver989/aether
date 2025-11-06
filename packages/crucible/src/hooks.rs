use std::{path::PathBuf, pin::Pin};

use crate::EvalMessage;

pub struct HookInput {
    pub working_directory: PathBuf,
    pub messages: Vec<EvalMessage>,
}

pub type HookResult = Result<(), Box<dyn std::error::Error>>;

/// Trait for eval lifecycle hooks (useful for running setup functions)
pub trait Hook: Send + Sync {
    fn run(&self, input: HookInput) -> Pin<Box<dyn Future<Output = HookResult> + Send>>;
}

// Implement for closures that return futures
impl<F, Fut> Hook for F
where
    F: Fn(HookInput) -> Fut + Send + Sync,
    Fut: Future<Output = HookResult> + Send + 'static,
{
    fn run(&self, input: HookInput) -> Pin<Box<dyn Future<Output = HookResult> + Send>> {
        Box::pin(self(input))
    }
}

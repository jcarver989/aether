# Timeout and Resource Limits

**Priority:** 🔴 P0 - Critical
**Impact:** Critical
**Effort:** Medium
**Estimated LOC:** ~150

## Problem

Agents can currently run indefinitely, which creates several critical issues:

1. **Blocked Eval Runs**: A single stuck eval can block an entire run for hours
2. **Wasted Resources**: Infinite loops or stuck agents consume API credits unnecessarily
3. **Cost Explosion**: Without token limits, a single eval could cost hundreds of dollars
4. **Poor UX**: Users can't estimate how long eval runs will take

### Current Code

In `packages/crucible/src/evals/runner.rs`, there's no timeout mechanism around agent execution:

```rust
// Line ~168
let messages = Self::run_agent(&runner, &eval, &working_dir, agent_prompt.as_deref()).await?;
```

## Solution

Add configurable timeout and resource limits to `EvalsConfig`:

```rust
// In packages/crucible/src/evals/config.rs
pub struct EvalsConfig<J> {
    pub judge_llm: J,
    pub batch_size: Option<usize>,
    pub batch_delay: Option<Duration>,
    pub serve: bool,

    // NEW FIELDS
    pub timeout: Option<Duration>,        // Per-eval timeout
    pub max_agent_tokens: Option<usize>,  // Max tokens for agent LLM
    pub max_judge_tokens: Option<usize>,  // Max tokens for judge LLM
}

impl<J> EvalsConfig<J> {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_max_agent_tokens(mut self, max_tokens: usize) -> Self {
        self.max_agent_tokens = Some(max_tokens);
        self
    }

    pub fn with_max_judge_tokens(mut self, max_tokens: usize) -> Self {
        self.max_judge_tokens = Some(max_tokens);
        self
    }
}
```

### Wrap Agent Execution in Timeout

In `packages/crucible/src/evals/runner.rs`:

```rust
// Add timeout wrapper around run_agent
let timeout_duration = config.timeout.unwrap_or(Duration::from_secs(600)); // Default 10 min

let messages = match tokio::time::timeout(
    timeout_duration,
    Self::run_agent(&runner, &eval, &working_dir, agent_prompt.as_deref())
).await {
    Ok(Ok(messages)) => messages,
    Ok(Err(e)) => {
        // Agent failed
        tracing::error!("Agent execution failed: {}", e);
        return Err(e);
    }
    Err(_) => {
        // Timeout occurred
        tracing::error!("Agent execution timed out after {:?}", timeout_duration);
        return Err(RunError::Timeout {
            duration: timeout_duration,
            eval_name: eval.name.clone(),
        });
    }
};
```

### Pass Token Limits to Agent

Modify `AgentConfig` to include token limits:

```rust
// In packages/crucible/src/agents/agent_runner.rs
pub struct AgentConfig<'a> {
    pub working_directory: &'a Path,
    pub system_prompt: Option<&'a str>,
    pub task_prompt: &'a str,

    // NEW FIELD
    pub max_tokens: Option<usize>,
}
```

Update `AetherRunner` to respect token limits when creating the agent:

```rust
// In packages/crucible/src/agents/aether_runner.rs
// When creating the agent, pass max_tokens to the LLM config
let agent = Agent::new(self.llm.clone())
    .with_max_tokens(config.max_tokens)
    // ... rest of config
```

### Track Timeout Failures in Results

Extend `EvalResult` to track timeout failures:

```rust
// In packages/crucible/src/storage/models.rs
pub enum EvalResult {
    Started { id: Uuid, eval_name: String },
    Running { id: Uuid, eval_name: String },

    // NEW VARIANT
    TimedOut {
        id: Uuid,
        eval_name: String,
        duration: Duration,
    },

    Completed {
        id: Uuid,
        eval_name: String,
        passed: bool,
        assertions: Vec<EvalAssertionResult>,
        agent_diff: Option<GitDiff>,
        reference_diff: Option<GitDiff>,
    },
}
```

### Define Specific Error Types

Following the error-handling best practice (no `anyhow` or `Box<dyn Error>`), define specific error types:

```rust
// In packages/crucible/src/evals/runner.rs or error.rs
#[derive(Debug)]
pub enum RunError {
    Timeout {
        duration: Duration,
        eval_name: String,
    },
    AgentFailed {
        eval_name: String,
        error: String,
    },
    ConfigError(String),
    StorageError(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout { duration, eval_name } => {
                write!(f, "Eval '{}' timed out after {:?}", eval_name, duration)
            }
            Self::AgentFailed { eval_name, error } => {
                write!(f, "Agent failed for eval '{}': {}", eval_name, error)
            }
            Self::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            Self::StorageError(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for RunError {}

pub type Result<T> = std::result::Result<T, RunError>;
```

## Files to Change

1. `packages/crucible/src/evals/config.rs` - Add timeout/token limit fields
2. `packages/crucible/src/evals/runner.rs` - Add `RunError` enum and wrap execution in `tokio::time::timeout()`
3. `packages/crucible/src/agents/agent_runner.rs` - Add `max_tokens` to `AgentConfig`
4. `packages/crucible/src/agents/aether_runner.rs` - Pass token limits to LLM
5. `packages/crucible/src/storage/models.rs` - Add `TimedOut` variant to `EvalResult`

## Usage Example

```rust
let config = EvalsConfig::new(judge_llm)
    .with_timeout(Duration::from_secs(300))  // 5 minute timeout per eval
    .with_max_agent_tokens(100_000)          // Max 100k tokens for agent
    .with_max_judge_tokens(10_000)           // Max 10k tokens for judges
    .with_batch_size(5);

let runner = EvalRunner::new(agent_runner, store)
    .run_evals(evals, config)
    .await?;
```

## Benefits

1. **Predictability**: Eval runs have bounded execution time
2. **Cost Control**: Token limits prevent runaway API costs
3. **Better UX**: Users can estimate run duration and costs
4. **Fail Fast**: Stuck agents fail quickly instead of hanging forever
5. **CI/CD Ready**: Timeouts make evals suitable for CI pipelines

## Testing Strategy

Following the testing-fakes.md best practice (use "Fake", never "Mock"):

1. Create a `FakeAgentRunner` that sleeps indefinitely to test timeouts
2. Verify timeout triggers and returns `RunError::Timeout` with proper enum matching
3. Test that token limits are respected by agents using a `FakeAgentRunner` with configurable token counts
4. Verify timeout results are saved correctly to storage using in-memory `FakeResultsStore`

Example test:
```rust
#[tokio::test]
async fn test_timeout_triggers_after_duration() {
    let fake_runner = FakeAgentRunner::with_delay(Duration::from_secs(10));
    let fake_store = FakeResultsStore::new();

    let config = EvalsConfig::new(fake_judge)
        .with_timeout(Duration::from_secs(1));

    let runner = EvalRunner::new(fake_runner, fake_store);
    let result = runner.run_evals(vec![eval], config).await;

    // Pattern match on specific error type
    match result {
        Err(RunError::Timeout { duration, .. }) => {
            assert_eq!(duration, Duration::from_secs(1));
        }
        _ => panic!("Expected timeout error"),
    }
}
```

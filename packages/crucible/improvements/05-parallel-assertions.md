# Parallel Assertion Execution

**Priority:** 🟡 P1 - High
**Impact:** Medium
**Effort:** Low
**Estimated LOC:** ~80

## Problem

Assertions currently run sequentially in `packages/crucible/src/evals/runner.rs:168-174`:

```rust
for assertion in &eval.assertions {
    let result = match assertion {
        EvalAssertion::FileExists { path } => {
            assert_file_exists(&working_dir, path)
        }
        // ... other assertions
    };
    assertion_results.push(result);
}
```

### Performance Impact

**Example Eval:**
```rust
vec![
    EvalAssertion::command_succeeds("cargo test"),      // 30 seconds
    EvalAssertion::command_succeeds("cargo clippy"),    // 20 seconds
    EvalAssertion::command_succeeds("cargo fmt"),       // 5 seconds
    EvalAssertion::file_exists("output.json"),          // <1ms
    EvalAssertion::llm_judge(|ctx| { ... }),            // 5 seconds
]
```

**Sequential execution:** 30 + 20 + 5 + 0 + 5 = **60 seconds**
**Parallel execution:** max(30, 20, 5, 0, 5) = **30 seconds** (50% faster!)

For evals with many independent assertions, this compounds significantly.

## Solution

Run independent assertions in parallel while respecting dependencies.

### Approach 1: Simple Parallel (No Dependencies)

For the common case where all assertions are independent:

```rust
// In packages/crucible/src/evals/runner.rs

use futures::future::join_all;

// Replace sequential loop with parallel execution
let assertion_futures: Vec<_> = eval
    .assertions
    .iter()
    .map(|assertion| {
        let assertion = assertion.clone();
        let working_dir = working_dir.clone();
        let messages = messages.clone();
        let judge_llm = judge_llm.clone();

        async move {
            evaluate_assertion(
                &assertion,
                &working_dir,
                original_prompt,
                &messages,
                &judge_llm,
            )
            .await
        }
    })
    .collect();

// Execute all assertions concurrently
let assertion_results = join_all(assertion_futures).await;
```

### Approach 2: Dependency-Aware Parallel (Future Enhancement)

For cases where some assertions depend on others:

```rust
// In packages/crucible/src/evals/assertion.rs

#[derive(Debug, Clone)]
pub struct AssertionWithDeps {
    pub assertion: EvalAssertion,
    /// Indices of assertions that must complete before this one
    pub depends_on: Vec<usize>,
}

impl EvalAssertion {
    /// Mark this assertion as dependent on others
    pub fn with_dependency(self, dep_index: usize) -> AssertionWithDeps {
        AssertionWithDeps {
            assertion: self,
            depends_on: vec![dep_index],
        }
    }
}
```

Example usage:
```rust
let assertions = vec![
    EvalAssertion::file_exists("input.txt"),                    // 0
    EvalAssertion::command_succeeds("process.sh"),              // 1, depends on 0
    EvalAssertion::file_exists("output.txt").with_dependency(1), // 2, depends on 1
    EvalAssertion::llm_judge(|_| "...".to_string()),            // 3, independent
];

// Execution order:
// Wave 1: assertions[0] and assertions[3] (parallel)
// Wave 2: assertions[1] (after 0 completes)
// Wave 3: assertions[2] (after 1 completes)
```

### Implementation (Simple Parallel)

```rust
// In packages/crucible/src/evals/runner.rs

/// Evaluate a single assertion
async fn evaluate_assertion<J: StreamingModelProvider>(
    assertion: &EvalAssertion,
    working_dir: &WorkingDirectory,
    original_prompt: &str,
    messages: &[AgentRunnerMessage],
    judge_llm: &J,
) -> EvalAssertionResult {
    let working_dir_path = match working_dir {
        WorkingDirectory::Local { path } => path,
        WorkingDirectory::GitRepo { path, .. } => path,
    };

    match assertion {
        EvalAssertion::FileExists { path } => {
            assert_file_exists(working_dir_path, path)
        }

        EvalAssertion::FileMatches { path, content } => {
            assert_file_matches(working_dir_path, path, content)
        }

        EvalAssertion::CommandExitCode {
            command,
            expected_code,
        } => {
            assert_command_exit_code(working_dir_path, command, *expected_code).await
        }

        EvalAssertion::ToolCall {
            name,
            arguments,
            count,
        } => {
            assert_tool_call(name, arguments.as_ref(), count, messages).await
        }

        EvalAssertion::LLMJudge { prompt_builder } => {
            assert_llm_judge(
                working_dir,
                original_prompt,
                messages,
                &*prompt_builder,
                judge_llm,
            )
            .await
        }
    }
}

// In run_single_eval function, replace sequential loop:
async fn run_single_eval<R, T, J>(
    // ... params
) -> Result<EvalResult, Box<dyn std::error::Error>>
where
    R: AgentRunner,
    T: ResultsStore,
    J: StreamingModelProvider,
{
    // ... existing code ...

    tracing::info!("Running {} assertions in parallel", eval.assertions.len());

    let assertion_futures: Vec<_> = eval
        .assertions
        .iter()
        .enumerate()
        .map(|(idx, assertion)| {
            let assertion = assertion.clone();
            let working_dir = working_dir.clone();
            let original_prompt = eval.task_prompt.clone();
            let messages = messages.clone();
            let judge_llm = judge_llm.clone();

            let span = tracing::info_span!(
                "assertion",
                eval_id = %eval_id,
                assertion_idx = idx,
                assertion_type = %assertion
            );

            async move {
                evaluate_assertion(
                    &assertion,
                    &working_dir,
                    &original_prompt,
                    &messages,
                    &*judge_llm,
                )
                .await
            }
            .instrument(span)
        })
        .collect();

    let assertion_results = futures::future::join_all(assertion_futures).await;

    // ... rest of function
}
```

## Files to Change

1. `packages/crucible/src/evals/runner.rs` - Add `evaluate_assertion()` helper function
2. `packages/crucible/src/evals/runner.rs` - Replace sequential loop with `join_all()`
3. `packages/crucible/Cargo.toml` - Ensure `futures` dependency exists

## Edge Cases to Handle

### 1. Shared Resource Contention

Some assertions might compete for resources:

```rust
vec![
    EvalAssertion::command_succeeds("npm install"),  // Writes to node_modules/
    EvalAssertion::command_succeeds("npm test"),     // Reads from node_modules/
]
```

**Solution:** These should be sequential. In simple parallel mode, user controls this via assertion ordering. In dependency-aware mode, use `depends_on`.

### 2. File System Race Conditions

```rust
vec![
    EvalAssertion::command_succeeds("rm -rf output/"),
    EvalAssertion::file_exists("output/result.txt"),
]
```

**Solution:** Again, user should ensure proper ordering. These assertions are inherently dependent.

### 3. LLM Judge Rate Limiting

Multiple parallel LLM judge calls could trigger rate limits:

```rust
vec![
    EvalAssertion::llm_judge(|_| "...".to_string()),
    EvalAssertion::llm_judge(|_| "...".to_string()),
    EvalAssertion::llm_judge(|_| "...".to_string()),
    EvalAssertion::llm_judge(|_| "...".to_string()),
]
```

**Solution:** Add semaphore to limit concurrent LLM judge calls:

```rust
// Add to EvalsConfig
pub struct EvalsConfig<J> {
    // ... existing fields
    pub max_concurrent_llm_judges: Option<usize>,
}

// Use semaphore in run_single_eval
let judge_semaphore = Arc::new(Semaphore::new(
    config.max_concurrent_llm_judges.unwrap_or(3)
));

// Before each LLM judge call
let _permit = judge_semaphore.acquire().await?;
assert_llm_judge(...).await
// Permit dropped, semaphore released
```

## Benefits

1. **Faster Eval Runs**: 30-70% speedup for evals with multiple independent assertions
2. **Better Resource Utilization**: CPU cores utilized while waiting on I/O
3. **Scalability**: Large eval suites benefit more from parallelism
4. **No Breaking Changes**: Works transparently for existing evals

## Testing Strategy

1. Create eval with multiple slow assertions (use `sleep` commands)
2. Measure execution time with sequential vs parallel
3. Test assertions with shared resources don't race
4. Verify all assertions execute and results are correct
5. Test error handling (some assertions fail, others succeed)

## Optional: Configuration Flag

Allow users to disable parallel assertions if needed:

```rust
let config = EvalsConfig::new(judge_llm)
    .with_parallel_assertions(false);  // Force sequential
```

This provides an escape hatch for problematic evals.

# Comprehensive Test Coverage

**Priority:** 🟢 P2 - High (Long-term)
**Impact:** High (Quality & Maintainability)
**Effort:** High
**Estimated LOC:** ~1000+

## Problem

The crucible package has minimal test coverage:

```bash
$ find packages/crucible -name "*test*.rs" -o -name "tests" -type d
# Returns: No results
```

This creates several risks:

1. **Regressions**: Changes can break functionality without detection
2. **Unclear Contracts**: Tests document expected behavior
3. **Refactoring Fear**: Hard to refactor without confidence
4. **Edge Cases**: Undiscovered bugs in error paths
5. **Integration Issues**: Components may not work together correctly

## Solution

Implement comprehensive test coverage across all modules.

### Testing Strategy

#### Unit Tests (Per Module)

Each module should have:
- Happy path tests
- Error case tests
- Edge case tests
- Boundary condition tests

#### Integration Tests

Test interactions between components:
- Agent runner + assertions
- Storage + retrieval
- Server + SSE events

#### End-to-End Tests

Full eval runs with `FakeAgentRunner`:
- Simple passing eval
- Failing assertions
- Git-based evals
- Batching behavior
- Timeout scenarios

## Test Coverage Priorities

### Priority 1: Core Assertions (CRITICAL)

**File:** `packages/crucible/src/assertions.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // FileExists Tests

    #[test]
    fn test_file_exists_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let result = assert_file_exists(temp_dir.path(), "test.txt");
        assert!(result.is_success());
    }

    #[test]
    fn test_file_exists_failure() {
        let temp_dir = TempDir::new().unwrap();

        let result = assert_file_exists(temp_dir.path(), "missing.txt");
        assert!(!result.is_success());
        assert!(result.message().contains("does not exist"));
    }

    // FileMatches Tests

    #[test]
    fn test_file_matches_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello World").unwrap();

        let result = assert_file_matches(temp_dir.path(), "test.txt", "World");
        assert!(result.is_success());
    }

    #[test]
    fn test_file_matches_failure() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello World").unwrap();

        let result = assert_file_matches(temp_dir.path(), "test.txt", "Goodbye");
        assert!(!result.is_success());
    }

    #[test]
    fn test_file_matches_file_not_found() {
        let temp_dir = TempDir::new().unwrap();

        let result = assert_file_matches(temp_dir.path(), "missing.txt", "content");
        assert!(!result.is_success());
        assert!(result.message().contains("Failed to read"));
    }

    // CommandExitCode Tests

    #[tokio::test]
    async fn test_command_succeeds() {
        let temp_dir = TempDir::new().unwrap();

        let result = assert_command_exit_code(
            temp_dir.path(),
            "echo 'hello'",
            0,
        ).await;

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_command_fails_wrong_exit_code() {
        let temp_dir = TempDir::new().unwrap();

        let result = assert_command_exit_code(
            temp_dir.path(),
            "exit 1",
            0,
        ).await;

        assert!(!result.is_success());
        assert!(result.message().contains("exited with code 1"));
    }

    #[tokio::test]
    async fn test_command_respects_working_directory() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("test.txt"), "data").unwrap();

        let result = assert_command_exit_code(
            temp_dir.path(),
            "test -f test.txt",
            0,
        ).await;

        assert!(result.is_success());
    }

    // ToolCall Tests

    #[tokio::test]
    async fn test_tool_call_found() {
        let messages = vec![
            AgentRunnerMessage::ToolCall {
                name: "read_file".to_string(),
                arguments: r#"{"path": "test.txt"}"#.to_string(),
            },
        ];

        let result = assert_tool_call("read_file", None, &None, &messages).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_tool_call_not_found() {
        let messages = vec![
            AgentRunnerMessage::ToolCall {
                name: "write_file".to_string(),
                arguments: r#"{"path": "test.txt"}"#.to_string(),
            },
        ];

        let result = assert_tool_call("read_file", None, &None, &messages).await;
        assert!(!result.is_success());
    }

    #[tokio::test]
    async fn test_tool_call_with_exact_args() {
        let messages = vec![
            AgentRunnerMessage::ToolCall {
                name: "read_file".to_string(),
                arguments: r#"{"path": "test.txt"}"#.to_string(),
            },
        ];

        let expected = serde_json::json!({"path": "test.txt"});
        let result = assert_tool_call("read_file", Some(&expected), &None, &messages).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_tool_call_count_exact() {
        let messages = vec![
            AgentRunnerMessage::ToolCall {
                name: "read_file".to_string(),
                arguments: r#"{"path": "a.txt"}"#.to_string(),
            },
            AgentRunnerMessage::ToolCall {
                name: "read_file".to_string(),
                arguments: r#"{"path": "b.txt"}"#.to_string(),
            },
        ];

        let result = assert_tool_call(
            "read_file",
            None,
            &Some(ToolCallCount::Exact(2)),
            &messages,
        ).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_tool_call_count_at_least() {
        let messages = vec![
            AgentRunnerMessage::ToolCall {
                name: "read_file".to_string(),
                arguments: r#"{"path": "a.txt"}"#.to_string(),
            },
        ];

        let result = assert_tool_call(
            "read_file",
            None,
            &Some(ToolCallCount::AtLeast(1)),
            &messages,
        ).await;
        assert!(result.is_success());

        let result = assert_tool_call(
            "read_file",
            None,
            &Some(ToolCallCount::AtLeast(2)),
            &messages,
        ).await;
        assert!(!result.is_success());
    }

    // LLM Judge Tests (with Fake LLM)

    #[tokio::test]
    async fn test_llm_judge_binary_success() {
        // Use Fake (not Mock) following testing-fakes.md best practice
        let fake_llm = FakeLlm::new(r#"{"type": "binary", "success": true, "reason": "Good"}"#);

        let result = assert_llm_judge(
            &WorkingDirectory::empty().unwrap(),
            "Task prompt",
            &[],
            |_ctx| "Judge prompt".to_string(),
            &fake_llm,
        ).await;

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_llm_judge_binary_failure() {
        let fake_llm = FakeLlm::new(r#"{"type": "binary", "success": false, "reason": "Bad"}"#);

        let result = assert_llm_judge(
            &WorkingDirectory::empty().unwrap(),
            "Task prompt",
            &[],
            |_ctx| "Judge prompt".to_string(),
            &fake_llm,
        ).await;

        assert!(!result.is_success());
    }

    #[tokio::test]
    async fn test_llm_judge_numeric_pass() {
        let fake_llm = FakeLlm::new(
            r#"{"type": "numeric", "score": 8.5, "max_score": 10.0, "reason": "Good"}"#
        );

        let result = assert_llm_judge(
            &WorkingDirectory::empty().unwrap(),
            "Task prompt",
            &[],
            |_ctx| "Judge prompt".to_string(),
            &fake_llm,
        ).await;

        assert!(result.is_success()); // 8.5/10 = 85% > 70% threshold
    }

    #[tokio::test]
    async fn test_llm_judge_invalid_json() {
        let fake_llm = FakeLlm::new("not json");

        let result = assert_llm_judge(
            &WorkingDirectory::empty().unwrap(),
            "Task prompt",
            &[],
            |_ctx| "Judge prompt".to_string(),
            &fake_llm,
        ).await;

        assert!(!result.is_success());
        assert!(result.message().contains("invalid JSON"));
    }
}

/// Fake LLM for testing (follows testing-fakes.md pattern)
///
/// This is a Fake (not a Mock) - it provides realistic behavior using
/// in-memory state instead of external dependencies.
pub struct FakeLlm {
    responses: Vec<String>,
    call_count: Arc<AtomicUsize>,
}

impl FakeLlm {
    /// Create a Fake LLM that returns the same response every time
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            responses: vec![response.into()],
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Create a Fake LLM that returns different responses on successive calls
    pub fn with_responses(responses: Vec<String>) -> Self {
        Self {
            responses,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the number of times this Fake LLM was called
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl StreamingModelProvider for FakeLlm {
    async fn stream_response(&self, _ctx: &Context) -> impl Stream<Item = Result<LlmResponse>> {
        let index = self.call_count.fetch_add(1, Ordering::SeqCst);
        let response = self.responses.get(index).unwrap_or(&self.responses[0]).clone();

        stream::once(async move {
            Ok(LlmResponse::Text { chunk: response })
        })
    }
}
```

### Priority 2: Git Integration

**File:** `packages/crucible/src/git_repo.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone_repository() {
        let temp_dir = TempDir::new().unwrap();

        let repo = GitRepo::clone(
            "https://github.com/anthropics/anthropic-sdk-python",
            temp_dir.path(),
        ).unwrap();

        assert!(temp_dir.path().join(".git").exists());
    }

    #[test]
    fn test_checkout_commit() {
        // Setup test repo with known commit
        // ...
    }

    #[test]
    fn test_diff_between_commits() {
        // Create test repo with two commits
        // Verify diff contains expected changes
    }

    #[test]
    fn test_blobless_clone_efficiency() {
        // Verify blobless clone is faster/smaller than full clone
    }
}
```

### Priority 3: Runner & Batching

**File:** `packages/crucible/src/evals/runner.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single_eval_passes() {
        let fake_runner = FakeAgentRunner::new(vec![
            AgentRunnerMessage::AgentText("Creating file".to_string()),
            AgentRunnerMessage::Done,
        ]);

        let store = FileSystemStore::new(TempDir::new().unwrap().path())?;
        let judge = FakeLlm::new(r#"{"type": "binary", "success": true}"#);

        let eval = Eval::new(
            "test",
            "Create a file",
            WorkingDirectory::empty()?,
            vec![],
        );

        let runner = EvalRunner::new(fake_runner, store);
        let result = runner.run_evals(vec![eval], EvalsConfig::new(judge)).await?;

        assert_eq!(result.passed, 1);
    }

    #[tokio::test]
    async fn test_batching_respects_size() {
        // Create 10 evals with batch_size=3
        // Verify they run in 4 batches (3 + 3 + 3 + 1)
    }

    #[tokio::test]
    async fn test_batching_respects_delay() {
        // Verify delay between batches is respected
    }
}
```

### Priority 4: Storage

**File:** `packages/crucible/src/storage/file_store.rs`

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_save_and_retrieve_eval_result() {
        // Save result, retrieve it, verify equality
    }

    #[test]
    fn test_trace_event_parsing() {
        // Write JSONL, read it back, verify events
    }

    #[test]
    fn test_concurrent_writes() {
        // Multiple evals writing concurrently shouldn't corrupt data
    }
}
```

## Files to Create/Modify

1. `packages/crucible/src/assertions.rs` - Add `#[cfg(test)] mod tests`
2. `packages/crucible/src/git_repo.rs` - Add tests
3. `packages/crucible/src/evals/runner.rs` - Add tests
4. `packages/crucible/src/storage/file_store.rs` - Add tests
5. `packages/crucible/src/agents/fake_agent_runner.rs` - Enhance for testing
6. `packages/crucible/tests/` - NEW: Integration tests directory

## Test Infrastructure Needed

### Fake LLM Provider

Following the testing-fakes.md best practice, create a comprehensive `FakeLlm` (not "Mock"):

```rust
// In packages/crucible/src/testing/fake_llm.rs

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

/// Fake LLM provider for testing (follows testing-fakes.md pattern)
///
/// This is a Fake, not a Mock - it provides realistic behavior using
/// in-memory state instead of external dependencies.
pub struct FakeLlm {
    responses: Vec<String>,
    call_count: Arc<AtomicUsize>,
}

impl FakeLlm {
    /// Create a Fake LLM that returns the same response every time
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            responses: vec![response.into()],
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Create a Fake LLM that returns different responses on successive calls
    pub fn with_responses(responses: Vec<String>) -> Self {
        Self {
            responses,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the number of times this Fake LLM was called
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl StreamingModelProvider for FakeLlm {
    async fn stream_response(&self, _ctx: &Context) -> impl Stream<Item = Result<LlmResponse>> {
        let index = self.call_count.fetch_add(1, Ordering::SeqCst);
        let response = self.responses.get(index).unwrap_or(&self.responses[0]).clone();

        stream::once(async move {
            Ok(LlmResponse::Text { chunk: response })
        })
    }
}
```

### Fake Results Store

```rust
// In packages/crucible/src/testing/fake_store.rs

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Fake results store using in-memory HashMap (follows testing-fakes.md)
pub struct FakeResultsStore {
    results: Arc<Mutex<HashMap<Uuid, Vec<EvalResult>>>>,
    metadata: Arc<Mutex<HashMap<Uuid, RunMetadata>>>,
}

impl FakeResultsStore {
    pub fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(HashMap::new())),
            metadata: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the number of runs stored (useful for test assertions)
    pub fn run_count(&self) -> usize {
        self.results.lock().unwrap().len()
    }
}

impl ResultsStore for FakeResultsStore {
    // Implement trait methods using in-memory HashMap
    // ...
}
```

### Test Fixtures

```rust
// Common test evals
pub fn simple_file_eval() -> Eval { /* ... */ }
pub fn git_based_eval() -> Eval { /* ... */ }
pub fn failing_eval() -> Eval { /* ... */ }
```

## Benefits

1. **Confidence**: Refactor without fear of breakage
2. **Documentation**: Tests show how to use the API
3. **Bug Prevention**: Catch regressions early
4. **Design Feedback**: Tests reveal API awkwardness
5. **Contributor Onboarding**: Tests help new contributors understand code

## Testing Milestones

**Milestone 1:** Core assertions (all 5 types)
**Milestone 2:** Git operations
**Milestone 3:** Runner & batching
**Milestone 4:** Storage & retrieval
**Milestone 5:** Integration tests
**Milestone 6:** 80%+ code coverage

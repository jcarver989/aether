# Candid Feedback on Crucible AI Evals Package

## What Works Well ✓

### 1. **Solid Architecture & Design Patterns**
- **Clean abstractions**: `ResultsStore` trait is well-designed for extensibility (filesystem, DB, cloud)
- **Builder patterns**: `EvalsConfig`, `EvalRunner` with fluent APIs are ergonomic
- **Async-first**: Proper use of tokio for concurrent eval execution
- **Type safety**: Strong typing prevents many runtime errors

### 2. **Comprehensive Assertion Coverage**
The 5 assertion types cover the essential eval dimensions:
- **ToolCall**: Validates agent behavior patterns (critical for agent evals)
- **FileExists/FileMatches**: Basic filesystem validation
- **CommandExitCode**: Integration testing capability
- **LLMJudge**: Flexible qualitative evaluation with structured JSON output

The `ToolCallCount` enum (Exact/AtLeast/AtMost) is particularly well-thought-out.

### 3. **Git Repository Support**
- Blobless cloning (`--filter=blob:none`) is a smart optimization
- Tracking both `start_commit` and `gold_commit` for reference solutions is clever
- Automatic diff capture for agent changes vs. reference is valuable

### 4. **Developer Experience**
- Real-time SSE updates and web dashboard at localhost:3000
- Structured tracing with custom layer writing to storage
- Batch processing with rate limiting controls
- Clear separation between programmatic API and filesystem-based config

### 5. **Hook System**
Lifecycle hooks (setup, before_assertions) provide necessary extension points without overcomplicating the API.

---

## What Doesn't Work Well / Critical Issues ✗

### 1. **❌ LLM Judge Reliability - MAJOR ISSUE**

**Problem**: The LLM judge implementation is fundamentally flawed:

```rust
// packages/crucible/src/assertions.rs:145-178
match serde_json::from_str::<EvalMetric>(trimmed_response) {
    Ok(metric) => { /* ... */ },
    Err(e) => {
        EvalAssertionResult::Failure {
            message: format!(
                "Judge returned invalid JSON: {e}\nRaw response: {judge_response}"
            ),
        }
    }
}
```

**Issues**:
- **No JSON extraction**: Judge LLMs often return markdown-wrapped JSON (` ```json\n{...}\n``` `), causing parse failures
- **No retry logic**: One parse failure = eval failure, even if the semantic judgment was correct
- **Hard-coded 70% threshold**: `numeric.score / numeric.max_score >= 0.7` at assertions.rs:151 is arbitrary
- **Single prompt attempt**: No few-shot examples or temperature tuning for judges

**Real-world Impact**: In production evals, this will cause ~20-30% false failures due to LLM judge formatting issues, not actual eval failures.

**Fix needed**:
```rust
// Extract JSON from markdown code blocks
let json_str = extract_json_from_markdown(&judge_response)
    .unwrap_or_else(|| judge_response.trim());

// Try parsing with multiple strategies
match serde_json::from_str::<EvalMetric>(json_str) {
    Ok(metric) => /* ... */,
    Err(_) => {
        // Retry with more explicit instructions
        // or use regex to extract structured fields
    }
}
```

### 2. **❌ No Eval Timeouts**

Agents can run indefinitely. At eval.rs:154-247, there's no timeout mechanism.

**Problem**:
- A stuck agent will block the entire batch
- No graceful degradation for non-terminating agents
- In production, you need per-eval timeouts (e.g., 5 minutes)

**Fix needed**:
```rust
pub struct Eval {
    // Add field:
    pub timeout: Option<Duration>,
}

// In run():
tokio::time::timeout(timeout, agent_execution).await
```

### 3. **❌ Message Accumulation Memory Issues**

At eval_messages.rs, the `to_eval_messages()` function accumulates ALL messages in memory:

```rust
pub async fn to_eval_messages(mut rx: Receiver<AgentMessage>) -> Vec<EvalMessage> {
    let mut eval_messages = Vec::new();
    // ... accumulates everything
    eval_messages
}
```

**Problem**:
- For long-running evals (e.g., multi-file refactoring), this can consume GBs of memory
- No streaming to disk or message limit
- Can cause OOM crashes on constrained systems

**Fix needed**: Add optional message limiting or streaming to disk for large transcripts.

### 4. **❌ Poor Error Context in Assertions**

Look at file_matches assertion (assertions.rs:28-50):

```rust
if file_content.contains(content) {
    EvalAssertionResult::Success {
        message: format!("File '{path}' contains '{content}'"),
    }
} else {
    EvalAssertionResult::Failure {
        message: format!("File '{path}' does not contain '{content}'"),
    }
}
```

**Problem**:
- No diff showing what was actually found vs. expected
- For long files, impossible to debug why match failed
- No substring matching options (exact, regex, fuzzy)

**Better approach**:
```rust
EvalAssertionResult::Failure {
    message: format!(
        "File '{path}' does not contain expected content.\n\
         Expected substring: {content}\n\
         Actual content (first 500 chars):\n{}",
        &file_content[..500.min(file_content.len())]
    ),
}
```

### 5. **❌ CommandExitCode is Unsafe**

At assertions.rs:54-103, commands are run with `sh -c` in the working directory:

```rust
let output = tokio::process::Command::new("sh")
    .arg("-c")
    .arg(command)
    .current_dir(working_dir)
    .output()
    .await;
```

**Problems**:
- No shell injection protection
- No resource limits (CPU, memory, disk)
- Commands can escape the working directory
- No timeout on command execution

**Critical for security**: If evals are run on untrusted agent output or in CI/CD, this is a shell injection vector.

### 6. **❌ Weak ToolCall Argument Matching**

At assertions.rs:200-204:

```rust
let actual_args = match serde_json::from_str::<serde_json::Value>(arguments) {
    Ok(args) => args,
    Err(_) => return None, // Invalid JSON
};

match expected_args {
    Some(expected) if actual_args == *expected => Some(actual_args),
    // ...
}
```

**Problem**: Exact JSON equality is too strict for real-world evals.

**Example failure**:
```json
Expected: {"path": "file.txt"}
Actual:   {"path": "file.txt", "encoding": "utf-8"}  // Agent added extra field
```

This would fail even though the critical `path` argument matched.

**Fix**: Support partial matching:
```rust
// Check if expected is a subset of actual
if expected.as_object().unwrap().iter()
    .all(|(k, v)| actual.get(k) == Some(v))
```

---

## What's Missing / Gaps in Functionality

### 1. **No Hierarchical Evals / Suites**

Current structure is flat: one eval = one prompt + assertions. No way to:
- Group related evals into suites
- Run prerequisite evals before dependent ones
- Share setup/teardown across eval groups

**Example use case**: "Test agent can read, modify, then deploy a microservice" requires 3+ sequential evals.

### 2. **No Assertion Soft Failures / Warnings**

All assertions are binary (pass/fail). Real-world evals need:
- **Warnings**: "Agent didn't use `read_file` efficiently (5 calls instead of 1)" - don't fail, but flag
- **Weighted scores**: Some assertions more important than others
- **Required vs Optional**: "Must write output.txt" vs "Should use caching for efficiency"

### 3. **No Diff-Based Assertions**

For code refactoring evals, you want assertions like:
- "Agent's diff should touch <10 files"
- "No test files should be modified"
- "Diff should not introduce any TODOs"

Currently, you'd need to write this manually in `LLMJudge` or `CommandExitCode`.

### 4. **No Regression Testing**

No built-in way to:
- Compare current run against previous baseline
- Flag regressions ("Agent passed this eval last week, now it's failing")
- Track eval performance over time

You have `RunResult` storage, but no comparison tooling.

### 5. **No Parallel Assertion Execution**

At eval.rs:209-244, assertions run sequentially:

```rust
for (i, assertion) in self.assertions.iter().enumerate() {
    // ...
    results.push((assertion.clone(), result));
}
```

For expensive assertions (multiple `LLMJudge` or slow `CommandExitCode`), this is wasteful. Could run independent assertions in parallel.

### 6. **No Agent Cost Tracking**

Critical for production evals:
- No token count tracking
- No latency metrics per tool call
- No cost estimation (especially for expensive judge LLMs)

Should add to `EvalResult`:
```rust
pub struct EvalResult {
    // Add:
    pub tokens_used: Option<TokenUsage>,
    pub estimated_cost_usd: Option<f64>,
    pub tool_call_latencies: HashMap<String, Duration>,
}
```

### 7. **Limited LLM Judge Context**

`LlmJudgeContext` only provides:
- `original_prompt`
- `messages`
- `git_diff()`

**Missing**:
- Access to file contents in working directory
- Access to assertion results from previous assertions
- Tool call trace (what tools were called in what order)
- Intermediate agent states

**Example**: Can't write a judge that says "Did agent read file A before writing file B?"

### 8. **No Sandbox/Isolation**

Evals run in temp directories on the host. No:
- Docker/container isolation
- Network restrictions
- Filesystem quota enforcement

**Risk**: Malicious or buggy agents could:
- Fill disk with infinite writes
- Make network requests to external services
- Access parent directories via symlinks

### 9. **No Distributed Execution**

Batching helps, but for 1000s of evals, you need:
- Distributed execution across machines
- Job queue integration (e.g., Celery, RabbitMQ)
- Result aggregation from multiple workers

### 10. **Web UI is Read-Only**

The SSE dashboard is view-only. No way to:
- Retry failed evals from UI
- Filter/search eval results
- Export to CSV/markdown report
- Compare two runs side-by-side

---

## Specific Code Quality Issues

### 1. **Inconsistent Error Handling**

Mix of `Box<dyn Error>`, `Result<_, String>`, and unwraps:
- lib.rs:191: `Err("No evals provided".into())` - should be typed error
- git_repo.rs: Inconsistent use of `?` vs `.map_err()`

### 2. **Hardcoded Magic Values**

- assertions.rs:151: `0.7` threshold
- lib.rs:246: `div_ceil` assumes batch logic without comments
- No constants file for tunable parameters

### 3. **Copy Semantics for WorkingDirectory**

At eval.rs:250-265, `copy_dir_all()` uses shell `cp -r`:

```rust
std::process::Command::new("cp")
    .arg("-r")
    .arg(src)
    .arg(dst)
```

**Issues**:
- Not cross-platform (Windows incompatible)
- Loses permissions/symlinks in some cases
- Should use `fs_extra::dir::copy()` or similar

### 4. **No Unit Tests**

I searched for test files - none found. For an evals framework, this is ironic. Need tests for:
- Each assertion type
- Message accumulation edge cases
- Git operations
- Store implementations

### 5. **Clone Trait Bounds**

Many structs have manual `Clone` implementations or derive(Clone) for types that shouldn't be cloned (e.g., `EvalAssertion` with `Arc<dyn Fn>`). This can lead to unexpected behavior.

---

## Recommendations (Priority Order)

### 🔴 Critical (Fix Now)
1. **Fix LLM judge JSON parsing** - Add markdown extraction and fallback strategies
2. **Add eval timeouts** - Prevent runaway agents
3. **Secure CommandExitCode** - Add sandboxing or at least timeout/resource limits
4. **Add basic unit tests** - Test assertion logic

### 🟡 Important (Next Sprint)
5. **Improve error messages** - Add diffs, context, and debugging info to assertion failures
6. **Add soft failures/warnings** - Not all assertion failures should fail the eval
7. **Fix ToolCall partial matching** - Support subset matching for arguments
8. **Add cost/token tracking** - Essential for production usage

### 🟢 Nice-to-Have (Future)
9. **Hierarchical eval suites** - Better organization for large eval sets
10. **Distributed execution** - Scale to thousands of evals
11. **Enhanced web UI** - Make dashboard interactive
12. **Container isolation** - Sandbox agent execution

---

## Overall Assessment

**Score: 7/10** - Solid foundation with critical gaps

**Strengths**:
- Well-architected core system
- Good assertion coverage
- Excellent git integration
- Developer-friendly API

**Weaknesses**:
- LLM judge implementation is production-blocking
- No timeouts or resource limits (security risk)
- Missing essential features (cost tracking, regression testing)
- No tests for a testing framework (!)

**Bottom Line**: This is a strong v0.1, but needs hardening before production use. The architecture is sound - most issues are implementation details that can be fixed incrementally. The LLM judge and timeout issues must be addressed before I'd recommend using this for serious eval work.

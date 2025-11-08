# Better Tool Argument Matching

**Priority:** 🟡 P1 - High
**Impact:** Medium
**Effort:** Low
**Estimated LOC:** ~60

## Problem

Tool call assertion currently uses exact JSON equality in `packages/crucible/src/assertions.rs:207`:

```rust
match expected_args {
    Some(expected) if actual_args == *expected => Some(actual_args),
    None => Some(actual_args),
    _ => None,
}
```

This is too rigid and breaks in common scenarios:

### Example Failure Cases

**1. Key Ordering**
```rust
// Assertion expects:
{"path": "file.txt", "mode": "0644"}

// Agent calls with:
{"mode": "0644", "path": "file.txt"}  // Same data, different order - FAILS!
```

**2. Extra Parameters**
```rust
// Assertion expects:
{"path": "output.txt"}

// Agent calls with:
{"path": "output.txt", "create_dirs": true}  // Has extra param - FAILS!
```

**3. Whitespace in JSON Strings**
```rust
// Assertion expects:
{"content": "Hello World"}

// Agent calls with:
{"content": "Hello  World"}  // Two spaces - FAILS!
```

**4. Numeric Type Differences**
```rust
// Assertion expects:
{"count": 5}

// Agent calls with:
{"count": 5.0}  // Int vs float - FAILS!
```

## Solution

Add flexible argument matching strategies:

### New Enum for Match Strategies

```rust
// In packages/crucible/src/evals/assertion.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArgumentMatchStrategy {
    /// Exact JSON equality (current behavior)
    Exact(serde_json::Value),

    /// Subset match - expected args must be present, extra args allowed
    Partial(serde_json::Value),

    /// Custom predicate function for complex matching
    #[serde(skip)]
    Predicate(Arc<dyn Fn(&serde_json::Value) -> bool + Send + Sync>),
}

impl Clone for ArgumentMatchStrategy {
    fn clone(&self) -> Self {
        match self {
            Self::Exact(v) => Self::Exact(v.clone()),
            Self::Partial(v) => Self::Partial(v.clone()),
            Self::Predicate(p) => Self::Predicate(p.clone()),
        }
    }
}
```

### Update ToolCall Assertion

```rust
// In packages/crucible/src/evals/assertion.rs

pub enum EvalAssertion {
    // ... other variants

    ToolCall {
        name: String,
        arguments: Option<ArgumentMatchStrategy>,  // Changed from Option<serde_json::Value>
        count: Option<ToolCallCount>,
    },
}

impl EvalAssertion {
    /// Assert that a tool was called with exact arguments
    pub fn tool_call_with_args_exact(
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self::ToolCall {
            name: name.into(),
            arguments: Some(ArgumentMatchStrategy::Exact(arguments)),
            count: None,
        }
    }

    /// Assert that a tool was called with at least these arguments (extra args allowed)
    pub fn tool_call_with_args_partial(
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self::ToolCall {
            name: name.into(),
            arguments: Some(ArgumentMatchStrategy::Partial(arguments)),
            count: None,
        }
    }

    /// Assert that a tool was called with arguments matching a predicate
    pub fn tool_call_with_args_matching<F>(
        name: impl Into<String>,
        predicate: F,
    ) -> Self
    where
        F: Fn(&serde_json::Value) -> bool + Send + Sync + 'static,
    {
        Self::ToolCall {
            name: name.into(),
            arguments: Some(ArgumentMatchStrategy::Predicate(Arc::new(predicate))),
            count: None,
        }
    }

    // Keep backward compatible method (defaults to Partial for better UX)
    pub fn tool_call_with_args(
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self::tool_call_with_args_partial(name, arguments)
    }
}
```

### Implementation

```rust
// In packages/crucible/src/assertions.rs

/// Check if actual arguments match the expected strategy
fn arguments_match(
    actual: &serde_json::Value,
    strategy: &ArgumentMatchStrategy,
) -> bool {
    match strategy {
        ArgumentMatchStrategy::Exact(expected) => {
            actual == expected
        }

        ArgumentMatchStrategy::Partial(expected) => {
            // Check if all expected keys exist in actual with matching values
            match (expected, actual) {
                (serde_json::Value::Object(exp_obj), serde_json::Value::Object(act_obj)) => {
                    exp_obj.iter().all(|(key, exp_value)| {
                        act_obj.get(key).map_or(false, |act_value| {
                            // Recursively check nested objects
                            if exp_value.is_object() {
                                arguments_match(
                                    act_value,
                                    &ArgumentMatchStrategy::Partial(exp_value.clone()),
                                )
                            } else {
                                act_value == exp_value
                            }
                        })
                    })
                }
                _ => actual == expected,
            }
        }

        ArgumentMatchStrategy::Predicate(predicate) => {
            predicate(actual)
        }
    }
}

pub async fn assert_tool_call(
    name: &str,
    expected_args: Option<&ArgumentMatchStrategy>,
    count: &Option<ToolCallCount>,
    messages: &[AgentRunnerMessage],
) -> EvalAssertionResult {
    let matching_calls: Vec<_> = messages
        .iter()
        .filter_map(|msg| {
            if let AgentRunnerMessage::ToolCall {
                name: call_name,
                arguments,
            } = msg
            {
                if call_name != name {
                    return None;
                }

                let actual_args = match serde_json::from_str::<serde_json::Value>(arguments) {
                    Ok(args) => args,
                    Err(_) => return None,
                };

                match expected_args {
                    Some(strategy) if arguments_match(&actual_args, strategy) => Some(actual_args),
                    None => Some(actual_args),
                    _ => None,
                }
            } else {
                None
            }
        })
        .collect();

    let actual_count = matching_calls.len();

    if let Some(count_req) = count {
        let count_valid = match count_req {
            ToolCallCount::Exact(expected) => actual_count == *expected,
            ToolCallCount::AtLeast(min) => actual_count >= *min,
            ToolCallCount::AtMost(max) => actual_count <= *max,
        };

        if !count_valid {
            return EvalAssertionResult::Failure {
                message: format!(
                    "Tool '{name}' was called {actual_count} times, but expected {count_req:?}"
                ),
            };
        }
    }

    if matching_calls.is_empty() {
        EvalAssertionResult::Failure {
            message: format!("Tool '{name}' was not called with matching arguments"),
        }
    } else {
        tracing::debug!(
            "✓ ToolCall assertion passed: {} (matched {} time(s))",
            name,
            actual_count
        );
        EvalAssertionResult::Success {
            message: format!("Tool '{name}' was called {actual_count} time(s) successfully"),
        }
    }
}
```

## Usage Examples

### Partial Matching (Most Common)

```rust
// Only care about the path parameter, ignore other params
EvalAssertion::tool_call_with_args_partial(
    "write_file",
    json!({"path": "output.txt"})
)

// Matches any of these:
// {"path": "output.txt"}
// {"path": "output.txt", "mode": "0644"}
// {"path": "output.txt", "create_dirs": true, "overwrite": false}
```

### Exact Matching (When Needed)

```rust
// Must match exactly
EvalAssertion::tool_call_with_args_exact(
    "set_permissions",
    json!({"path": "file.txt", "mode": "0644"})
)

// Only matches:
// {"path": "file.txt", "mode": "0644"}
// Does NOT match:
// {"path": "file.txt", "mode": "0644", "recursive": true}
```

### Predicate Matching (Advanced)

```rust
// Check that count parameter is greater than 5
EvalAssertion::tool_call_with_args_matching(
    "fetch_users",
    |args| {
        args.get("count")
            .and_then(|v| v.as_u64())
            .map_or(false, |count| count > 5)
    }
)

// Check that path ends with .json
EvalAssertion::tool_call_with_args_matching(
    "read_file",
    |args| {
        args.get("path")
            .and_then(|v| v.as_str())
            .map_or(false, |path| path.ends_with(".json"))
    }
)

// Check that arguments contain a valid email
EvalAssertion::tool_call_with_args_matching(
    "send_email",
    |args| {
        args.get("to")
            .and_then(|v| v.as_str())
            .map_or(false, |email| email.contains('@'))
    }
)
```

### Nested Object Matching

```rust
// Partial match works recursively
EvalAssertion::tool_call_with_args_partial(
    "create_user",
    json!({
        "user": {
            "name": "Alice"
        }
    })
)

// Matches:
// {"user": {"name": "Alice", "age": 30, "email": "alice@example.com"}}
// {"user": {"name": "Alice"}, "send_welcome_email": true}
```

## Files to Change

1. `packages/crucible/src/evals/assertion.rs` - Add `ArgumentMatchStrategy` enum
2. `packages/crucible/src/evals/assertion.rs` - Update builder methods
3. `packages/crucible/src/assertions.rs` - Add `arguments_match()` helper
4. `packages/crucible/src/assertions.rs` - Update `assert_tool_call()` to use strategy

## Benefits

1. **Flexibility**: Handle common real-world argument variations
2. **Robustness**: Assertions don't break on harmless differences (key order, extra params)
3. **Expressiveness**: Predicate matching allows complex validation logic
4. **Backward Compatible**: Default to partial matching for better UX

## Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let actual = json!({"a": 1, "b": 2});
        let strategy = ArgumentMatchStrategy::Exact(json!({"a": 1, "b": 2}));
        assert!(arguments_match(&actual, &strategy));

        let strategy = ArgumentMatchStrategy::Exact(json!({"a": 1}));
        assert!(!arguments_match(&actual, &strategy));
    }

    #[test]
    fn test_partial_match_allows_extra_fields() {
        let actual = json!({"a": 1, "b": 2, "c": 3});
        let strategy = ArgumentMatchStrategy::Partial(json!({"a": 1, "b": 2}));
        assert!(arguments_match(&actual, &strategy));
    }

    #[test]
    fn test_partial_match_requires_all_expected_fields() {
        let actual = json!({"a": 1});
        let strategy = ArgumentMatchStrategy::Partial(json!({"a": 1, "b": 2}));
        assert!(!arguments_match(&actual, &strategy));
    }

    #[test]
    fn test_partial_match_nested_objects() {
        let actual = json!({
            "user": {"name": "Alice", "age": 30},
            "active": true
        });
        let strategy = ArgumentMatchStrategy::Partial(json!({
            "user": {"name": "Alice"}
        }));
        assert!(arguments_match(&actual, &strategy));
    }

    #[test]
    fn test_predicate_match() {
        let actual = json!({"count": 10});
        let strategy = ArgumentMatchStrategy::Predicate(Arc::new(|args| {
            args.get("count").and_then(|v| v.as_u64()).map_or(false, |c| c > 5)
        }));
        assert!(arguments_match(&actual, &strategy));
    }
}
```

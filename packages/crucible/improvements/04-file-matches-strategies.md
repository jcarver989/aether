# Improve FileMatches with Multiple Strategies

**Priority:** 🟡 P1 - High
**Impact:** High
**Effort:** Low
**Estimated LOC:** ~100

## Problem

The current `FileMatches` assertion only supports simple substring matching:

```rust
// In packages/crucible/src/assertions.rs:32
if file_content.contains(content) {
    // Success
}
```

This is too rigid for real-world use cases:

### Example Failures

**1. Whitespace Sensitivity**
```rust
// Assertion expects:
"Hello World"

// Agent writes:
"Hello  World"  // Two spaces - assertion fails!
```

**2. No Pattern Matching**
```rust
// Want to verify file contains a valid JSON array
// Can't express "any JSON array", must hardcode exact content
```

**3. No Semantic Matching**
```rust
// Want to verify generated code "implements a REST API"
// Can't express intent, must match exact string
```

## Solution

Add multiple matching strategies to `FileMatches`:

### New Enum for Match Strategies

```rust
// In packages/crucible/src/evals/assertion.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileMatchStrategy {
    /// Substring match (current behavior)
    Contains(String),

    /// Regex pattern match
    Regex(String),

    /// Exact equality (entire file must match)
    Exact(String),

    /// LLM judges if file content is correct
    LlmJudge {
        prompt: String,
        /// Optional: expected file type for context
        file_type: Option<String>,
    },

    /// JSON equality (parse and compare ignoring whitespace/ordering)
    JsonEquals(serde_json::Value),

    /// File is valid for given format (json, yaml, toml, etc.)
    ValidFormat(String),
}
```

### Update EvalAssertion

```rust
pub enum EvalAssertion {
    FileExists {
        path: String,
    },
    FileMatches {
        path: String,
        strategy: FileMatchStrategy,  // Changed from `content: String`
    },
    // ... other variants
}

impl EvalAssertion {
    /// Assert that a file contains substring (backward compatible)
    pub fn file_contains(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self::FileMatches {
            path: path.into(),
            strategy: FileMatchStrategy::Contains(content.into()),
        }
    }

    /// Assert that a file matches a regex pattern
    pub fn file_matches_regex(path: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::FileMatches {
            path: path.into(),
            strategy: FileMatchStrategy::Regex(pattern.into()),
        }
    }

    /// Assert that a file exactly equals the given content
    pub fn file_equals(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self::FileMatches {
            path: path.into(),
            strategy: FileMatchStrategy::Exact(content.into()),
        }
    }

    /// Use LLM to judge if file content is correct
    pub fn file_llm_judge(
        path: impl Into<String>,
        prompt: impl Into<String>,
        file_type: Option<String>,
    ) -> Self {
        Self::FileMatches {
            path: path.into(),
            strategy: FileMatchStrategy::LlmJudge {
                prompt: prompt.into(),
                file_type,
            },
        }
    }

    /// Assert that a file contains valid JSON equal to expected value
    pub fn file_json_equals(path: impl Into<String>, expected: serde_json::Value) -> Self {
        Self::FileMatches {
            path: path.into(),
            strategy: FileMatchStrategy::JsonEquals(expected),
        }
    }

    /// Assert that a file is valid in the given format
    pub fn file_valid_format(path: impl Into<String>, format: impl Into<String>) -> Self {
        Self::FileMatches {
            path: path.into(),
            strategy: FileMatchStrategy::ValidFormat(format.into()),
        }
    }
}
```

### Implementation

```rust
// In packages/crucible/src/assertions.rs

pub async fn assert_file_matches<J: StreamingModelProvider>(
    working_dir: &Path,
    path: &str,
    strategy: &FileMatchStrategy,
    judge_llm: Option<&J>,
) -> EvalAssertionResult {
    let file_path = working_dir.join(path);

    let file_content = match std::fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            return EvalAssertionResult::Failure {
                message: format!("Failed to read file '{path}': {e}"),
            };
        }
    };

    match strategy {
        FileMatchStrategy::Contains(substring) => {
            if file_content.contains(substring) {
                EvalAssertionResult::Success {
                    message: format!("File '{path}' contains '{substring}'"),
                }
            } else {
                EvalAssertionResult::Failure {
                    message: format!("File '{path}' does not contain '{substring}'"),
                }
            }
        }

        FileMatchStrategy::Regex(pattern) => {
            match regex::Regex::new(pattern) {
                Ok(re) => {
                    if re.is_match(&file_content) {
                        EvalAssertionResult::Success {
                            message: format!("File '{path}' matches pattern '{pattern}'"),
                        }
                    } else {
                        EvalAssertionResult::Failure {
                            message: format!("File '{path}' does not match pattern '{pattern}'"),
                        }
                    }
                }
                Err(e) => {
                    EvalAssertionResult::Failure {
                        message: format!("Invalid regex pattern '{pattern}': {e}"),
                    }
                }
            }
        }

        FileMatchStrategy::Exact(expected) => {
            if file_content == *expected {
                EvalAssertionResult::Success {
                    message: format!("File '{path}' exactly matches expected content"),
                }
            } else {
                EvalAssertionResult::Failure {
                    message: format!(
                        "File '{path}' does not match.\nExpected:\n{}\n\nActual:\n{}",
                        expected, file_content
                    ),
                }
            }
        }

        FileMatchStrategy::LlmJudge { prompt, file_type } => {
            let judge_llm = match judge_llm {
                Some(llm) => llm,
                None => {
                    return EvalAssertionResult::Failure {
                        message: "LLM judge requested but no judge LLM provided".to_string(),
                    };
                }
            };

            let file_type_context = file_type
                .as_ref()
                .map(|ft| format!("File type: {ft}\n\n"))
                .unwrap_or_default();

            let full_prompt = format!(
                "{file_type_context}File path: {path}\n\nFile content:\n```\n{file_content}\n```\n\n{prompt}\n\n{}",
                BinaryMetric::json_schema()
            );

            // Use existing LLM judge logic
            assert_llm_judge_for_file(judge_llm, &full_prompt).await
        }

        FileMatchStrategy::JsonEquals(expected) => {
            match serde_json::from_str::<serde_json::Value>(&file_content) {
                Ok(actual) => {
                    if actual == *expected {
                        EvalAssertionResult::Success {
                            message: format!("File '{path}' contains expected JSON"),
                        }
                    } else {
                        EvalAssertionResult::Failure {
                            message: format!(
                                "File '{path}' JSON mismatch.\nExpected:\n{}\n\nActual:\n{}",
                                serde_json::to_string_pretty(expected).unwrap(),
                                serde_json::to_string_pretty(&actual).unwrap()
                            ),
                        }
                    }
                }
                Err(e) => {
                    EvalAssertionResult::Failure {
                        message: format!("File '{path}' is not valid JSON: {e}"),
                    }
                }
            }
        }

        FileMatchStrategy::ValidFormat(format) => {
            let is_valid = match format.as_str() {
                "json" => serde_json::from_str::<serde_json::Value>(&file_content).is_ok(),
                "yaml" | "yml" => serde_yaml::from_str::<serde_yaml::Value>(&file_content).is_ok(),
                "toml" => toml::from_str::<toml::Value>(&file_content).is_ok(),
                "xml" => quick_xml::de::from_str::<serde_json::Value>(&file_content).is_ok(),
                _ => {
                    return EvalAssertionResult::Failure {
                        message: format!("Unknown format '{format}'"),
                    };
                }
            };

            if is_valid {
                EvalAssertionResult::Success {
                    message: format!("File '{path}' is valid {format}"),
                }
            } else {
                EvalAssertionResult::Failure {
                    message: format!("File '{path}' is not valid {format}"),
                }
            }
        }
    }
}
```

## Usage Examples

### Regex Pattern Matching

```rust
// Verify file contains a valid UUID
EvalAssertion::file_matches_regex(
    "config.json",
    r#""id":\s*"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}""#
)

// Verify file contains valid email addresses
EvalAssertion::file_matches_regex(
    "users.txt",
    r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}"
)
```

### LLM Judge for Semantic Matching

```rust
// Verify generated code implements required functionality
EvalAssertion::file_llm_judge(
    "src/api.rs",
    "Does this file implement a REST API server with GET and POST endpoints for user management?",
    Some("rust".to_string())
)

// Verify documentation quality
EvalAssertion::file_llm_judge(
    "README.md",
    "Is this README well-structured with clear installation and usage instructions?",
    Some("markdown".to_string())
)
```

### JSON Equality

```rust
// Verify exact JSON structure (ignoring whitespace)
EvalAssertion::file_json_equals(
    "output.json",
    json!({
        "users": [
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"}
        ]
    })
)
```

### Format Validation

```rust
// Just verify file is valid JSON, don't check content
EvalAssertion::file_valid_format("config.json", "json")

// Verify YAML is parseable
EvalAssertion::file_valid_format("k8s/deployment.yaml", "yaml")
```

## Files to Change

1. `packages/crucible/src/evals/assertion.rs` - Add `FileMatchStrategy` enum and builder methods
2. `packages/crucible/src/assertions.rs` - Implement all matching strategies
3. `packages/crucible/Cargo.toml` - Add dependencies: `regex`, `serde_yaml`, `toml`, `quick-xml`

## Benefits

1. **Flexibility**: Support wide variety of file validation needs
2. **Backward Compatible**: Old `file_matches()` still works via `Contains` strategy
3. **Semantic Validation**: LLM judge for complex, subjective criteria
4. **Format Validation**: Easy checks for valid JSON/YAML/TOML/XML
5. **Pattern Matching**: Regex for flexible string patterns

## Testing Strategy

1. Unit tests for each strategy with various inputs
2. Test regex edge cases (invalid patterns, special characters)
3. Test JSON equality with different orderings
4. Test LLM judge with mock responses
5. Test format validation with malformed files

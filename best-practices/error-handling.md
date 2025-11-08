# Error Handling

**Location**: `packages/aether/src/llm/error.rs`, `packages/aether/src/agent/error.rs`

## Pattern

```rust
#[derive(Debug)]
pub enum LlmError {
    MissingApiKey,
    ToolParameterParsing { tool_name: String, error: String },
    HttpError(reqwest::Error),
}

impl From<reqwest::Error> for LlmError {
    fn from(e: reqwest::Error) -> Self { Self::HttpError(e) }
}

pub type Result<T> = std::result::Result<T, LlmError>;
```

## Core Principles

- **Use enum variants for error types** - Enables pattern matching, no `anyhow`/`color-eyre`
- **Struct variants for context** - `{ tool_name: String, error: String }` includes relevant data
- **Implement `From<ExternalError>`** - Enables `?` operator for foreign types
- **Type alias for Result** - `pub type Result<T> = std::result::Result<T, DomainError>`
- **Implement Display** - User-friendly error messages

## Why

- Callers can pattern match on specific errors
- Compile-time exhaustiveness checking
- Type system enforces error handling
- No stringly-typed errors

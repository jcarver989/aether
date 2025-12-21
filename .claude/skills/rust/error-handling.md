# Rust Error Handling

Patterns for effective error handling in Rust.

## Contents

- [Library vs Application](#library-vs-application-distinction) - thiserror vs anyhow
- [The thiserror Crate](#the-thiserror-crate) - library error types
- [When to Use panic!](#when-to-use-panic)
- [Error Context with anyhow](#error-context-with-anyhow)

## Library vs Application Distinction

### Libraries: Use Concrete Error Types

Libraries should emit detailed, typed errors that consumers can match on:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyLibraryError {
    #[error("failed to read configuration: {0}")]
    Config(#[from] std::io::Error),

    #[error("invalid format in {file}: {reason}")]
    InvalidFormat { file: String, reason: String },

    #[error("connection failed after {attempts} attempts")]
    ConnectionFailed { attempts: u32 },
}
```

### Applications: Use `anyhow`

Applications can use `anyhow` for convenience:

```rust
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let config = read_config()
        .context("failed to read configuration")?;

    process(&config)
        .with_context(|| format!("failed to process {}", config.name))?;

    Ok(())
}
```

## The `thiserror` Crate

Use `thiserror` to reduce boilerplate:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DataError {
    // Simple message
    #[error("data not found")]
    NotFound,

    // With interpolation
    #[error("invalid data at position {position}")]
    Invalid { position: usize },

    // Wrapping another error (auto-implements From)
    #[error("IO error")]
    Io(#[from] std::io::Error),

    // Transparent (delegates Display and source to inner error)
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}
```

## When to Use `panic!`

**DO use panic for:**
- Programmer errors / violated invariants
- Unrecoverable states that indicate bugs
- Tests and examples
- Prototyping (replace with proper errors later)

**DON'T use panic for:**
- Expected error conditions
- User input validation
- Network/IO failures
- Anything a library consumer might want to handle

```rust
// Good: panic for invariant violation
fn get_element(slice: &[i32], index: usize) -> i32 {
    assert!(index < slice.len(), "index out of bounds: bug in caller");
    slice[index]
}

// Bad: panic for expected condition
fn parse_config(input: &str) -> Config {
    serde_json::from_str(input).unwrap()  // Don't do this!
}

// Good: return Result for expected failures
fn parse_config(input: &str) -> Result<Config, ConfigError> {
    serde_json::from_str(input).map_err(ConfigError::from)
}
```

## Error Context with `anyhow`

Add context to understand error chains:

```rust
use anyhow::{Context, Result};

fn process_file(path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let data: Data = serde_json::from_str(&content)
        .context("failed to parse JSON")?;

    validate(&data)
        .context("validation failed")?;

    Ok(())
}
```

## Quick Reference

| Context | Use | Example |
|---------|-----|---------|
| Library code | `thiserror` | `#[derive(Error, Debug)]` |
| Application code | `anyhow` | `anyhow::Result<T>` |
| Adding context | `.context()` | `.context("failed to parse")?` |
| Bugs/invariants | `panic!` | `assert!(condition)` |

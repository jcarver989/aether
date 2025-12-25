# CLAUDE.md

You are an expert senior Rust engineer.

## Project Overview

Aether lightweight AI coding assistant written in Rust that provides Claude Code-like functionality through a modular architecture. It leverages the Model Context Protocol (MCP) for dynamic tool discovery and integration, supporting both OpenRouter and Ollama as LLM providers.

## Build and Development Commands

```bash
# Build the project
cargo build

# Run the project
cargo run

# Run tests
cargo test

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Coding Style

### Generics

1. Prefer generics over dynamic boxing where possible, e.g. `fn foo<T: Animal>(animal: T)` over `fn foo(boo: Box<dyn Animal>)`
2. Prefer the more compact `T: Animal` syntax vs `where T: Animal` where possible

### Async Rust

Do not write async traits like so:

```rust
trait Foo {
    fn bar<'a>(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>
}
```

Instead, write them like this:

```rust
trait Foo {
    async fn bar(&self) -> Resul<()>
}
```

### File organization

- Put `pub` sturcts, traits, functions, and types at the top of the file.
- Put private functions below `pub` constructs, near the bottom of the file.
- The reader's eye should flow from the most important, high-level things (top) to the less important nitty-gritty details (bottom). Example `eval_runner.rs` should have `pub struct EvalRunner` near the top

### Writing tests

1. Never use the word `Mock`. Use `Fake` instead.

2. Prefer writing `Fake` objects that mimic the _real_ behavior of the thing being faked using an in-memory implementation instead of producing side-effects. For example, a `FakeFilesystem` should work just like a real file system, but `write_file()` might write to a `HashMap` instead of the file-system.

### Error handling

1. Never add `anyhow` or `color-eyere` as dependencies. Use standard Rust `enum`'s instead, e.g. `enum ApiError { ... }`
2. Prefer using specific enum types over `Box<dyn std::error::Error>` as the later makes it impossible for the caller to pattern match on specific errors.
3. Leverage `map`, `flat_map`, `and_then` etc to flatten nested `match` statements like `Ok(Ok(foo)) => {...}`.

### Imports

1. Use the type's name `fn foo() -> Boo` over the fully qualified name `fn foo() -> some::module::that_makes_it_hard_to_read::Boo`

## Best Practices

Be sure to consult your skills to check for relevant best practices.

## CRITICAL - ALWAYS FOLLOW THIS WORKFLOW

1. Always write tests to prove your code works
2. If fixing a bug, write a failing test  FIRST, BEFORE making changes. Then make the test(s) pass.
3. ALWAYS run tests before declaring your work done -- you may have broken something

---
name: writing-rust
description: Rust best practices, idiomatic patterns, and project conventions. Use when writing Rust code, reviewing PRs, debugging issues, or working with error handling, traits, concurrency, or cargo tooling.
---

# Rust Best Practices

Core principles for writing idiomatic, safe, and efficient Rust code.

## Contents

- [Type System](#type-system) - newtypes, enums, conversions
- [Option and Result](#option-and-result) - combinators, `?` operator
- [Error Handling](#error-handling) → [error-handling.md](./error-handling.md)
- [Traits](#traits) → [traits.md](./traits.md)
- [Memory and Ownership](#memory-and-ownership)
- [Concurrency](#concurrency) → [concurrency.md](./concurrency.md)
- [Iterators](#iterators)
- [Tooling](#tooling) → [tooling.md](./tooling.md)
- [Project Conventions](#project-conventions-mcp-gateway)
- [Anti-Patterns](#anti-patterns-to-avoid)
- [Testing](./testing-fakes.md)
- [Builders](./builder-pattern.md)
- [Types](./type-safety.md)

## Type System

### Use Types to Encode Semantics

- **Make invalid states inexpressible** - design types so only valid combinations are possible at compile time
- **Use newtype pattern** (`struct Wrapper(T)`) to add semantic meaning (units, domain concepts)
- **Mark newtypes with `#[repr(transparent)]`** if binary compatibility matters
- **Prefer descriptive structs over tuples** with non-distinctive types

### Enums Over Booleans

```rust
// Bad: unclear what true/false mean
print_page(true, false);

// Good: self-documenting
print_page(Sides::Both, Output::Grayscale);
```

- Use enums for mutually exclusive states
- Exhaustive matching is enforced by the compiler
- Use enums with associated data (algebraic data types) to encode invariants

### Type Conversions

- **Implement `From<T>`** for infallible conversions (not `Into` - it's auto-generated)
- **Implement `TryFrom<T>`** for fallible conversions (returns `Result`)
- **Prefer `from`/`into` over `as` casts** - safer and more explicit
- Use `Into<T>` trait bounds on generics to accept both wrapped and unwrapped types

## Option and Result

### Prefer Transformations Over Match

```rust
// Instead of explicit match:
let value = match opt {
    Some(v) => transform(v),
    None => default,
};

// Use combinators:
let value = opt.map(transform).unwrap_or(default);
```

Key combinators: `.map()`, `.and_then()`, `.map_err()`, `.ok_or()`, `.unwrap_or_default()`

### The ? Operator

- Use `?` for error propagation - cleaner than explicit match
- Implement `From<SubError>` for your error type to enable automatic conversion

## Error Handling

See [error-handling.md](./error-handling.md) for detailed patterns.

**Quick reference:**
- Libraries: emit concrete, detailed error types (use enums or `thiserror`)
- Applications: use `anyhow` for convenience across heterogeneous errors
- Always implement `std::error::Error` for your error types

## Traits

See [traits.md](./traits.md) for derive macros and trait implementation guidance.

**Essential derives for most types:**
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
```

**For hashable/orderable types:**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
```

## Memory and Ownership

### Prefer Owned Data in Structs

- Avoid lifetime parameters on structs when possible
- Allocating/cloning often leads to simpler, more maintainable code
- Optimize only when benchmarking shows it's necessary

### Smart Pointers for Complex Ownership

- `Rc<RefCell<T>>` - single-threaded shared ownership with interior mutability
- `Arc<Mutex<T>>` - multi-threaded shared ownership
- `Weak<T>` - back-references that don't prevent cleanup

### Avoid Self-Referential Structures

- Fundamentally incompatible with Rust's move semantics
- Use indexing or the `Pin` type if absolutely necessary

## Concurrency

See [concurrency.md](./concurrency.md) for detailed patterns.

**Key points:**
- Rust prevents data races but NOT deadlocks
- Use `Send` and `Sync` marker traits
- Prefer message passing over shared state when possible
- Thread-safe alternatives: `Arc` instead of `Rc`, `Mutex`/`RwLock` instead of `RefCell`

## Iterators

### Prefer Iterator Transformations

```rust
// Instead of:
let mut result = Vec::new();
for item in collection {
    if condition(&item) {
        result.push(transform(item));
    }
}

// Use:
let result: Vec<_> = collection
    .into_iter()
    .filter(condition)
    .map(transform)
    .collect();
```

### Common Patterns

- `collect::<Result<Vec<_>, _>>()` to convert `Vec<Result>` to `Result<Vec>`

## Tooling

See [tooling.md](./tooling.md) for cargo, clippy, testing, and CI guidance.

**Essential commands:**
- `cargo fmt` - format code
- `cargo clippy` - lint for issues
- `cargo check` - fast syntax check
- `cargo test` - run tests
- `cargo doc` - generate documentation

## Project Conventions (mcp-gateway)

This project uses specific crates and patterns:

| Layer | Crate | Notes |
|-------|-------|-------|
| HTTP | Axum | Handlers, extractors, middleware |
| Database | sqlx | Run `just db prepare` after schema changes |
| gRPC | Tonic | ConnectRPC for frontend communication |
| MCP | rmcp | MCP protocol implementation |
| Errors | thiserror | Structured error types in gateway |

**Key patterns:**
- Use `anyhow` in binary/main, `thiserror` in library code
- Run `just ci` before committing (fmt, clippy, test)
- Migrations in `/migrations` - see `/migrations/CLAUDE.md` for standards

## Anti-Patterns to Avoid

- **Don't use `.unwrap()` in library code** - prefer `?` or proper error handling
- **Don't use `panic!` for expected errors** - reserve for programmer errors/invariant violations
- **Don't write `unsafe` without exhausting safe alternatives**
- **Don't obsess over zero-copy** - simple, correct code is often better
- **Don't add lifetime annotations to structs unless necessary**

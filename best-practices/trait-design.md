# Trait Design

**Location**: `packages/aether/src/llm/provider.rs`, `packages/crucible/src/eval_runner.rs`

## Pattern 1: Blanket Implementations for Smart Pointers

```rust
impl<T: StreamingModelProvider + ?Sized> StreamingModelProvider for Box<T> {
    fn stream_completion(&self, req: CompletionRequest) -> impl Future<...> {
        (**self).stream_completion(req)
    }
}

impl<T: StreamingModelProvider> StreamingModelProvider for Arc<T> {
    fn stream_completion(&self, req: CompletionRequest) -> impl Future<...> {
        (**self).stream_completion(req)
    }
}
```

**Enables**: `Vec<Box<dyn StreamingModelProvider>>`, `Arc<MyProvider>`, plugin architectures

## Pattern 2: Dependency Injection with Generics

```rust
pub struct EvalRunner<R: AgentRunner, S: ResultsStore> {
    runner: R,
    store: S,
}

// Production: EvalRunner::new(ProductionRunner, FileStore)
// Test: EvalRunner::new(FakeRunner, InMemoryStore)
```

**Use generics over trait objects for**: Zero-cost abstraction, testability, monomorphization

## Pattern 3: Modern Async Traits

```rust
pub trait AgentRunner {
    fn run(&self, config: AgentConfig<'_>) -> impl Future<Output = Result<T>> + Send;
}

// NOT: #[async_trait] or Pin<Box<dyn Future>>
```

**Why**: No macros, no boxing, explicit bounds, better errors

## Core Principles

- **Blanket impl for `Box<T>` and `Arc<T>`** - Enables dynamic dispatch when needed
- **Generic type parameters for DI** - `struct Foo<T: Trait>` not `Box<dyn Trait>`
- **`impl Future + Send` syntax** - Modern async traits without async-trait macro
- **Use `?Sized` for Box** - Supports `Box<dyn Trait>`
- **Always `+ Send` on async traits** - Required for tokio::spawn

## When to Choose

- **Generic parameters**: Known at compile time, performance critical, testing
- **Trait objects**: Heterogeneous collections, runtime plugin loading

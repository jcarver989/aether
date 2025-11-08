# Middleware Pattern

**Location**: `packages/aether/src/agent/middleware.rs`

## Pattern: Parallel Middleware with Type Erasure

```rust
pub struct Middleware {
    handlers: Vec<Arc<dyn Fn(Event) -> BoxFuture<'static, Action> + Send + Sync>>,
}

impl Middleware {
    pub fn add_handler<F, Fut>(&mut self, handler: F)
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Action> + Send + 'static,
    {
        let boxed = Arc::new(move |event| Box::pin(handler(event)) as BoxFuture<_>);
        self.handlers.push(boxed);
    }

    pub async fn run(&self, event: Event) -> Action {
        let futures = self.handlers.iter().map(|h| h(event.clone()));
        let results = join_all(futures).await;

        // "Any block wins" - security first
        if results.iter().any(|r| matches!(r, Action::Block)) {
            return Action::Block;
        }
        Action::Allow
    }
}
```

## Core Principles

- **Type erasure with Arc** - Store heterogeneous closures in Vec via `Arc<dyn Fn>`
- **Parallel execution** - `join_all()` runs handlers concurrently
- **"Any block wins" semantic** - Security-first: single Block overrides all Allows
- **Arc not Box** - Enables `Clone` for sharing across threads
- **BoxFuture** - Type erases different future types

## Usage

```rust
middleware.add_handler(|event| async move {
    if DANGEROUS_TOOLS.contains(&event.tool_name) {
        Action::Block
    } else {
        Action::Allow
    }
});
```

**Use for**: Security validation, logging, rate limiting, metrics - cross-cutting concerns

## Why

- Handlers don't block each other (parallel I/O)
- Can mix different closure types in same Vec
- Security-first: any handler can veto an action
- Clean separation of concerns

# Streaming and Async

**Location**: `packages/aether/src/llm/anthropic/streaming.rs`, `packages/crucible/src/agent_runner.rs`

## Pattern: Stateful Stream Processing

```rust
use async_stream::stream;

pub fn transform_stream(input: impl Stream<Item = Event>) -> impl Stream<Item = Result> {
    stream! {
        let mut state = HashMap::new();  // Local state persists across items

        for await event in input {
            // Update state
            state.insert(event.id, event.data);

            // Yield transformed item
            yield process(&state, event);
        }

        // Cleanup - handle remaining state
        for (id, data) in state {
            yield finalize(id, data);
        }
    }
}
```

**Use for**: Accumulating partial data, correlating multi-part messages (start/delta/stop), deduplication

## Core Principles

- **async_stream::stream! macro** - Readable async generators with local state
- **Local state with HashMap** - Track information across stream items
- **for await** - Async iteration over input stream
- **yield** - Emit items to output stream
- **Cleanup after loop** - Process remaining state when input exhausted

## Stream Combinators

```rust
stream
    .filter(|x| future::ready(!x.is_empty()))
    .map(|x| process(x))
    .buffer_unordered(10)  // Process 10 concurrently (out of order)
    .collect().await
```

## Why

- Sequential code that's easy to understand
- Lazy evaluation - processes as items arrive
- Composable transformations
- Type-safe with compile-time checking

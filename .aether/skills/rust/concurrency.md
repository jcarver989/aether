# Rust Concurrency

Patterns for safe concurrent programming in Rust.

## Contents

- [Thread-Safe Alternatives](#thread-safe-alternatives)
- [Deadlock Prevention](#deadlock-prevention) (critical)
- [Scoped Threads](#scoped-threads-rust-163)
- [Async Considerations](#async-considerations)

## Key Principle

Rust prevents data races but **NOT deadlocks**. The deadlock prevention section is critical.

## Thread-Safe Alternatives

| Single-threaded | Multi-threaded |
|-----------------|----------------|
| `Rc<T>` | `Arc<T>` |
| `RefCell<T>` | `Mutex<T>` or `RwLock<T>` |
| `Cell<T>` | `Atomic*` types |

Use `Arc<Mutex<T>>` for shared mutable state. Use `Arc<RwLock<T>>` when reads dominate.

## Deadlock Prevention

Rust does NOT prevent deadlocks. Follow these guidelines:

### 1. Put Related Data Under a Single Mutex

```rust
// Bad: two separate locks
struct BadState {
    users: Mutex<HashMap<UserId, User>>,
    sessions: Mutex<HashMap<SessionId, Session>>,
}

// Good: single lock for related data
struct GoodState {
    inner: Mutex<StateInner>,
}

struct StateInner {
    users: HashMap<UserId, User>,
    sessions: HashMap<SessionId, Session>,
}
```

### 2. Keep Lock Scopes Small and Obvious

```rust
// Bad: lock held across complex operation
fn bad_update(state: &Mutex<State>) {
    let mut guard = state.lock().unwrap();
    guard.field = expensive_computation();  // Lock held during computation
    call_external_service(&guard);  // Lock held during I/O
}

// Good: minimize lock scope
fn good_update(state: &Mutex<State>) {
    let value = expensive_computation();  // No lock held

    {
        let mut guard = state.lock().unwrap();
        guard.field = value;
    }  // Lock released

    let snapshot = {
        let guard = state.lock().unwrap();
        guard.clone()
    };
    call_external_service(&snapshot);  // No lock held
}
```

### 3. Never Return a MutexGuard

```rust
// Bad: caller controls lock lifetime
fn bad_get_data(&self) -> MutexGuard<Data> {
    self.data.lock().unwrap()
}

// Good: return owned or cloned data
fn good_get_data(&self) -> Data {
    self.data.lock().unwrap().clone()
}

// Or use a callback pattern
fn with_data<R>(&self, f: impl FnOnce(&Data) -> R) -> R {
    let guard = self.data.lock().unwrap();
    f(&guard)
}
```

### 4. Avoid Invoking Closures with Locks Held

```rust
// Bad: unknown code runs with lock held
fn bad_process<F: FnOnce(&mut Data)>(state: &Mutex<Data>, f: F) {
    let mut guard = state.lock().unwrap();
    f(&mut guard);  // f might try to acquire another lock!
}

// Good: clone, modify, replace
fn good_process<F: FnOnce(&mut Data)>(state: &Mutex<Data>, f: F) {
    let mut data = state.lock().unwrap().clone();
    drop(state.lock());  // Release lock
    f(&mut data);  // Process without lock
    *state.lock().unwrap() = data;  // Update
}
```

### 5. Include Deadlock Detection in CI

Use `parking_lot::deadlock` or `no_deadlocks` crate in tests.

## Scoped Threads (Rust 1.63+)

Borrow data without `Arc`:

```rust
thread::scope(|s| {
    s.spawn(|| println!("{:?}", data));  // Borrow directly
    s.spawn(|| println!("{:?}", data));  // Multiple borrows OK
});  // Joined before data goes out of scope
```

## Async Considerations

**Never hold locks across `await` points** - this blocks the executor.

```rust
// BAD: lock held across await
let guard = mutex.lock().await;
do_async_work().await;  // Other tasks blocked!

// GOOD: clone and release
let data = mutex.lock().await.clone();
drop(guard);
do_async_work().await;
```

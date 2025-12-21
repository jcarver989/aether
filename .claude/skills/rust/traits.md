# Rust Traits and Derive Macros

Guidelines for implementing and deriving traits.

## Contents

- [Essential Derive Patterns](#essential-derive-pattern)
- [When NOT to Derive](#when-not-to-derive)
- [Manual Implementation Patterns](#manual-implementation-patterns)
- [The Orphan Rule](#the-orphan-rule)
- [Conversion Traits](#conversion-traits)

## Essential Derive Pattern

For most types:
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
```

For small, copyable types:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
```

For enums with a default:
```rust
#[derive(Default)]
enum Status {
    #[default]
    Pending,
    Active,
}
```

## When NOT to Derive

| Trait | Skip when... |
|-------|-------------|
| `Clone` | Type owns unique resources (handles, connections, secrets) |
| `Copy` | Type is large (implicit copies become expensive) |
| `Default` | No sensible default exists |
| `PartialEq`/`Eq` | Some fields shouldn't affect equality (caches, timestamps) |
| `Hash` | Custom `Eq` implementation (must keep Hash consistent) |
| `Debug` | Type contains sensitive data (passwords, keys) |

## Manual Implementation Patterns

### Excluding fields from Eq/Hash

**Critical:** If `x == y`, then `hash(x) == hash(y)`. Keep Eq and Hash consistent.

```rust
#[derive(Debug)]
struct User {
    id: u64,
    name: String,
    cached_hash: Option<u64>,  // Exclude from Eq/Hash
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.name == other.name
    }
}
impl Eq for User {}

impl std::hash::Hash for User {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.name.hash(state);  // Same fields as Eq
    }
}
```

### Custom Ord (excluding fields)

```rust
impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch)  // Exclude label
            .cmp(&(other.major, other.minor, other.patch))
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

## The Orphan Rule

Can't implement foreign traits for foreign types. **Workaround:** newtype pattern.

```rust
struct MyVec<T>(Vec<T>);  // Wrap foreign type
impl<T: fmt::Display> fmt::Display for MyVec<T> { /* ... */ }
```

## Conversion Traits

### AsRef for flexible parameters

Accept multiple types without allocation:

```rust
fn read_file(path: impl AsRef<Path>) -> io::Result<String> {
    std::fs::read_to_string(path.as_ref())
}

// Works with &str, String, Path, PathBuf
read_file("config.toml")?;
read_file(path_buf)?;
```

### From/TryFrom

- Implement `From<T>` for infallible conversions (`Into` auto-generated)
- Implement `TryFrom<T>` for fallible conversions

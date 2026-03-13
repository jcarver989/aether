# Rust Tooling

Cargo commands, testing, CI, and development tools.

## Contents

- [Cargo Plugins](#useful-cargo-plugins)
- [Clippy Configuration](#clippy)
- [Testing Patterns](#testing)
- [CI Best Practices](#ci-best-practices)

## Useful Cargo Plugins

Install with `cargo install <name>`:

| Tool | Description |
|------|-------------|
| `cargo-expand` | Show macro expansion output |
| `cargo-tarpaulin` | Code coverage generation |
| `cargo-udeps` | Detect unused dependencies |
| `cargo-deny` | Check licenses, duplicates, security advisories |
| `cargo-semver-checks` | Verify semantic versioning correctness |
| `cargo-watch` | Re-run commands on file changes |
| `cargo-nextest` | Faster test runner |

## Clippy

### Running Clippy

```bash
# Basic run
cargo clippy

# Treat warnings as errors (for CI)
cargo clippy -- -D warnings

# Fix auto-fixable issues
cargo clippy --fix

# Check all targets including tests
cargo clippy --all-targets
```

### Common Clippy Lints to Know

```rust
// Allow specific lint
#[allow(clippy::too_many_arguments)]
fn complex_function(...) {}

// Deny specific lint at crate level
#![deny(clippy::unwrap_used)]

// Configure in Cargo.toml
[lints.clippy]
unwrap_used = "deny"
expect_used = "warn"
```

### Useful Lint Groups

- `clippy::pedantic` - More strict lints
- `clippy::nursery` - New, potentially unstable lints
- `clippy::cargo` - Cargo.toml best practices

## Testing

### Property Testing with proptest

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_roundtrip(s in "\\PC*") {
        let encoded = encode(&s);
        let decoded = decode(&encoded)?;
        prop_assert_eq!(s, decoded);
    }
}
```

## Fuzzing

### Using cargo-fuzz

```bash
# Install
cargo install cargo-fuzz

# Initialize
cargo fuzz init

# Add fuzz target
cargo fuzz add my_target

# Run fuzzer
cargo +nightly fuzz run my_target
```

```rust
// fuzz/fuzz_targets/my_target.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = my_crate::parse(s);
    }
});
```

## CI Best Practices

### Essential CI Steps

```yaml
# Example GitHub Actions workflow
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      # Fast checks first
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings

      # Build and test
      - run: cargo build --all-targets
      - run: cargo test

      # Documentation
      - run: cargo doc --no-deps
```

### CI Principles

1. **Don't waste human time** - automate everything
2. **Be ruthless with flaky tests** - fix immediately
3. **Run `cargo fmt --check`** to enforce style
4. **Run `cargo clippy -- -D warnings`** to fail on warnings
5. **Test every feature combination** if crate has features
6. **Check MSRV** (minimum supported Rust version) if declared
7. **No exceptions to CI checks** - once there's an accepted failure, regressions sneak in

### Feature Testing

```bash
# Test with no features
cargo test --no-default-features

# Test with all features
cargo test --all-features

# Test specific feature combinations
cargo test --features "feature1,feature2"
```

### Toolchain Pinning

Create `rust-toolchain.toml` for reproducible builds:

```toml
[toolchain]
channel = "1.75.0"
components = ["rustfmt", "clippy"]
```

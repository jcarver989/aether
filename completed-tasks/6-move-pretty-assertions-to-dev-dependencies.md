# Task: Move pretty_assertions to Dev Dependencies

## Priority: Low

## Overview
The `pretty_assertions` crate is currently in the main `[dependencies]` section but should be in `[dev-dependencies]` since it's only used for testing.

## Current State
In `Cargo.toml`:
```toml
[dependencies]
# ... other deps ...
pretty_assertions = "1.4.1"
# ... more deps ...

[dev-dependencies]
tempfile = "3.8"
```

## Expected Behavior
Testing-only dependencies should be in `[dev-dependencies]` to:
- Reduce binary size for release builds
- Decrease compilation time for production builds
- Follow Rust best practices

## Implementation Steps

1. **Verify pretty_assertions is only used in tests**:
   ```bash
   # Check for usage in source files
   grep -r "pretty_assertions" src/
   
   # Check for usage in test files
   grep -r "pretty_assertions" tests/
   grep -r "assert_eq!" src/ tests/
   ```

2. **Move the dependency**:
   ```toml
   [dependencies]
   # Remove this line:
   # pretty_assertions = "1.4.1"
   
   [dev-dependencies]
   tempfile = "3.8"
   pretty_assertions = "1.4.1"  # Add here
   ```

3. **Verify the change**:
   ```bash
   # Clean build
   cargo clean
   
   # Check that tests still compile and run
   cargo test
   
   # Check that release build works and is smaller
   cargo build --release
   ```

4. **Check for any macro usage** that might need updating:
   - `assert_eq!` from pretty_assertions is a drop-in replacement
   - Should work transparently in test code

## Testing Requirements
- All tests must continue to pass
- Test output should still show pretty diffs on failures
- Release binary size should be slightly reduced
- No compilation errors in any configuration

## Success Criteria
- `pretty_assertions` is in `[dev-dependencies]`
- All tests pass
- Release builds don't include pretty_assertions code
- No functionality changes

## Benefits
1. **Smaller binaries**: Test code not included in release
2. **Faster builds**: Less code to compile for production
3. **Cleaner dependencies**: Clear separation of concerns
4. **Best practices**: Follows Rust community standards

## Common Issues to Watch For
- If any source files use pretty_assertions macros (they shouldn't)
- Import statements in test files might need `#[cfg(test)]`

## Example Test File Headers
```rust
// In test files, this should continue to work:
#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    
    #[test]
    fn test_something() {
        assert_eq!(1 + 1, 2);
    }
}
```

## Estimated Effort
15-30 minutes

## Dependencies
None - this is an independent task

## Notes
- This is a good "first issue" for someone new to the codebase
- Similar audit could be done for other dependencies
- Check if any other test-only dependencies are in the wrong section
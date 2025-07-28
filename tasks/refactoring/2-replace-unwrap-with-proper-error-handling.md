# Task: Replace unwrap() with Proper Error Handling

## Priority: Critical

## Overview
The codebase contains 174 instances of `unwrap()` across 15 files, which can cause the application to panic in production. These need to be replaced with proper error handling using Rust's Result/Option patterns.

## Current State
Files with high unwrap() usage:
- `src/config.rs`: 20 occurrences
- `src/components/input.rs`: 30 occurrences
- Tests: ~100+ occurrences (less critical but should be addressed)
- Various other source files

## Expected Behavior
- Production code should never panic on recoverable errors
- Use `?` operator for error propagation
- Use `match` or `if let` for explicit handling
- Use `expect()` with meaningful messages only when panic is the correct behavior
- Use `.unwrap_or()`, `.unwrap_or_else()`, or `.unwrap_or_default()` for providing defaults

## Implementation Steps

### Phase 1: Critical Path (src/ files)
1. **Audit each unwrap() in source files**:
   - Determine if the unwrap can ever fail
   - Classify as: safe to unwrap, needs error handling, or needs default value

2. **Replace based on classification**:
   ```rust
   // Before
   let value = some_option.unwrap();
   
   // After - when error should propagate
   let value = some_option.ok_or_else(|| anyhow!("Expected value not found"))?;
   
   // After - when default is acceptable
   let value = some_option.unwrap_or_default();
   
   // After - when panic is correct (rare)
   let value = some_option.expect("Critical: config must have value X");
   ```

3. **Common patterns to fix**:
   - Config loading: Should return Result with descriptive errors
   - String operations: Use proper UTF-8 handling
   - Collection access: Use `.get()` instead of indexing
   - Channel operations: Handle closed channel cases

### Phase 2: Test Files
1. **For test files**, unwrap() is more acceptable but consider:
   - Using `expect()` with descriptive messages
   - Using assertion macros that provide better error messages
   - Only keeping unwrap() for setup code where panic is appropriate

## Specific Areas of Focus

### src/config.rs (20 occurrences)
- Config parsing should return descriptive errors
- Missing config values should have helpful error messages
- Consider providing defaults where appropriate

### src/components/input.rs (30 occurrences)
- Cursor position calculations need bounds checking
- String slicing operations need UTF-8 boundary safety
- Use `.get()` for safe indexing into collections

## Testing Requirements
- Ensure no functionality is broken by the changes
- Add tests for error cases that were previously unwrapping
- Verify error messages are helpful and actionable
- Test edge cases: empty collections, invalid indices, malformed data

## Success Criteria
- Zero unwrap() calls in production code (src/)
- All error paths properly handled with meaningful messages
- No panics during normal operation
- Test files use expect() with descriptive messages where appropriate

## Guidelines
1. **Don't just replace unwrap() with expect("")** - add meaningful context
2. **Consider the user experience** - what error message would help them?
3. **Propagate errors up** to where they can be meaningfully handled
4. **Use domain-specific error types** where it makes sense

## Example Transformations

```rust
// Bad
let config = Config::load().unwrap();

// Good
let config = Config::load()
    .context("Failed to load configuration from default location")?;

// Bad
let char = input.chars().nth(position).unwrap();

// Good
let char = input.chars().nth(position)
    .ok_or_else(|| anyhow!("Cursor position {} is out of bounds", position))?;

// Bad
let line = lines[index].clone();

// Good
let line = lines.get(index)
    .ok_or_else(|| anyhow!("Line index {} is out of bounds (total lines: {})", index, lines.len()))?
    .clone();
```

## Estimated Effort
4-6 hours for source files
2-3 hours for test files

## Dependencies
- May require adding error context to function signatures
- Some functions may need to change from returning T to Result<T>
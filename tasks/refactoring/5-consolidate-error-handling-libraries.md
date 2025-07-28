# Task: Consolidate Error Handling Libraries

## Priority: Medium

## Overview
The project uses both `anyhow` and `thiserror` error handling crates. This is redundant and can lead to inconsistent error handling patterns. We should standardize on one approach.

## Current State
In `Cargo.toml`:
- `anyhow = "1.0"` - Provides flexible error handling for applications
- `thiserror = "1.0"` - Provides derive macros for custom error types

The codebase appears to primarily use `color_eyre::Result` but has both libraries as dependencies.

## Expected Behavior
Choose one error handling strategy:
- **For an application**: Use `anyhow` (or stick with `color_eyre` which is already used)
- **For a library**: Use `thiserror` for well-defined error types
- Since Aether is an application, we should remove unused error crates

## Analysis Needed

1. **Check current usage**:
   ```bash
   # Check for thiserror usage
   grep -r "thiserror" src/
   grep -r "#\[derive(.*Error.*)\]" src/
   
   # Check for anyhow usage  
   grep -r "anyhow" src/
   grep -r "use anyhow" src/
   
   # Check for color_eyre usage
   grep -r "color_eyre" src/
   ```

2. **Determine primary error handling pattern**

## Implementation Steps

### If `thiserror` is unused:
1. Remove from `Cargo.toml`
2. Run `cargo check` to ensure nothing breaks

### If `anyhow` is unused:
1. Remove from `Cargo.toml`
2. Run `cargo check` to ensure nothing breaks

### If both are used minimally:
1. Standardize on `color_eyre` (already used for application errors)
2. Replace any `anyhow::Result` with `color_eyre::Result`
3. Replace any `anyhow!()` with `eyre!()` or `color_eyre::eyre::eyre!()`
4. Remove both dependencies

### If custom error types are needed:
1. Keep `thiserror` only for defining specific error types
2. Remove `anyhow`
3. Use `color_eyre` for application-level error handling

## Best Practices for Rust Error Handling

1. **Applications** (like Aether):
   - Use `anyhow` or `color_eyre` for flexible error handling
   - Focus on good error messages rather than types
   - Easy error propagation with `?`

2. **Libraries**:
   - Use `thiserror` to define specific error types
   - Allow library users to handle specific errors
   - Maintain backwards compatibility

3. **Mixed approach**:
   - Use `thiserror` for domain-specific errors
   - Use `anyhow`/`color_eyre` at application boundaries

## Testing Requirements
- Ensure all error paths still work correctly
- Check that error messages remain informative
- Verify panic handler still works (in `src/errors.rs`)
- Run full test suite

## Success Criteria
- Only necessary error handling crates in dependencies
- Consistent error handling pattern throughout codebase
- No compilation warnings or errors
- Error messages remain helpful

## Example Migrations

```rust
// If migrating from anyhow to color_eyre
// Before
use anyhow::{Result, anyhow};
fn process() -> Result<()> {
    Err(anyhow!("Processing failed"))
}

// After
use color_eyre::Result;
fn process() -> Result<()> {
    Err(color_eyre::eyre::eyre!("Processing failed"))
}

// Or with report
use color_eyre::Result;
fn process() -> Result<()> {
    Err(color_eyre::Report::msg("Processing failed"))
}
```

## Estimated Effort
1-2 hours

## Dependencies
- Should be done before any major error handling improvements
- May affect error messages shown to users

## Notes
- Aether already uses `color_eyre` extensively
- The `src/errors.rs` file sets up `color_eyre` hooks
- This consolidation will make error handling more consistent
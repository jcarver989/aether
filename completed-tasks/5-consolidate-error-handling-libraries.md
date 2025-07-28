# Task: Consolidate Error Handling Libraries

## Priority: Medium

## Overview
The project uses both `anyhow` and `thiserror` error handling crates. This is redundant and can lead to inconsistent error handling patterns. We should standardize on one approach.

## Current State
In `Cargo.toml`:
- `anyhow = "1.0"` - Provides flexible error handling for applications
- `thiserror = "1.0"` - Provides derive macros for custom error types

The codebase should use `color_eyre::Result` going forward.

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

1. Standardize on `color_eyre` (already used for application errors)
2. Replace any `anyhow::Result` with `color_eyre::Result`
3. Replace any `anyhow!()` with `eyre!()` or `color_eyre::eyre::eyre!()`
4. Remove both dependencies

## Best Practices for Rust Error Handling

1. **Applications** (like Aether):
   - Use `color_eyre` for flexible error handling
   - Focus on good error messages rather than types
   - Easy error propagation with `?`


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

## Dependencies
- Should be done before any major error handling improvements
- May affect error messages shown to users

## Notes
- Aether already uses `color_eyre` extensively
- The `src/errors.rs` file sets up `color_eyre` hooks
- This consolidation will make error handling more consistent

# Task: Remove Dead Code Allowances

## Priority: Low

## Overview
The codebase has `#![allow(dead_code)]` directives in actively used modules, which suppresses compiler warnings about unused code. These should be removed to maintain code quality.

## Current State
Files with dead code allowances:
- `src/tui.rs:1`: `#![allow(dead_code)] // Remove this once you start using the code`
- `src/config.rs:1`: `#![allow(dead_code)] // Remove this once you start using the code`

Both files are actively used in the application.

## Expected Behavior
- Remove the `#![allow(dead_code)]` directives
- Address any resulting warnings by either:
  - Removing genuinely unused code
  - Adding `#[allow(dead_code)]` to specific items that are intentionally kept
  - Making private items public if they're part of the API

## Implementation Steps

1. **Remove the directives**:
   - Delete line 1 from both `src/tui.rs` and `src/config.rs`

2. **Run cargo check to identify warnings**:
   ```bash
   cargo check
   ```

3. **For each warning, determine the appropriate action**:
   
   a. **If code is unused and not needed**:
      - Delete it
   
   b. **If code is for future use**:
      - Add item-level attribute:
      ```rust
      #[allow(dead_code)]
      fn future_feature() {
          // Implementation
      }
      ```
   
   c. **If code should be public**:
      - Change visibility:
      ```rust
      pub fn previously_private() {
          // Implementation
      }
      ```

4. **Common patterns to check**:
   - Unused struct fields
   - Private functions only used in tests
   - Constants defined but not used
   - Enum variants that aren't matched

## Testing Requirements
- Ensure `cargo check` produces no warnings
- Verify all tests still pass
- Check that no functionality is broken
- Run `cargo clippy` to catch any additional issues

## Success Criteria
- No module-level `#![allow(dead_code)]` directives
- All compiler warnings addressed appropriately
- Code compiles without warnings
- No functional regressions

## Example Fixes

```rust
// Before
#![allow(dead_code)]

struct Config {
    used_field: String,
    unused_field: String, // Warning suppressed
}

// After - Option 1: Remove unused field
struct Config {
    used_field: String,
}

// After - Option 2: Mark as intentionally unused
struct Config {
    used_field: String,
    #[allow(dead_code)]
    unused_field: String, // Reserved for future feature X
}
```

## Benefits
1. **Code Quality**: Compiler helps identify genuinely unused code
2. **Maintenance**: Easier to identify what code is actually used
3. **Performance**: Smaller binary size by removing unused code
4. **Documentation**: Clear indication of what's intentionally kept vs forgotten

## Estimated Effort
30 minutes - 1 hour

## Dependencies
None - this can be done independently

## Notes
- This is a good task for getting familiar with the codebase
- May reveal opportunities for further cleanup
- Should be done after more critical refactoring tasks
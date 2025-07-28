# Task: Modernize Exit Code Usage

## Priority: Low

## Overview
The code uses `libc::EXIT_FAILURE` directly in the panic handler instead of Rust's modern `std::process::ExitCode` API, which is now stable and more idiomatic.

## Current State
In `src/errors.rs:47`:
```rust
std::process::exit(libc::EXIT_FAILURE);
```

## Expected Behavior
Use Rust's standard library types instead of libc constants:
- More portable across platforms
- More idiomatic Rust
- Type-safe exit codes
- Better integration with Rust ecosystem

## Implementation Steps

1. **Replace libc constant with standard library**:
   ```rust
   // Before
   std::process::exit(libc::EXIT_FAILURE);
   
   // After - Option 1: Direct value
   std::process::exit(1);
   
   // After - Option 2: Use ExitCode (if refactoring main)
   std::process::ExitCode::FAILURE
   ```

2. **Consider removing libc dependency** if this is the only usage:
   ```bash
   # Check for other libc usage
   grep -r "libc::" src/
   ```

3. **If refactoring more extensively**, consider:
   ```rust
   // Modern main function
   fn main() -> std::process::ExitCode {
       if let Err(e) = run() {
           eprintln!("Error: {}", e);
           return std::process::ExitCode::FAILURE;
       }
       std::process::ExitCode::SUCCESS
   }
   ```

## Options for Implementation

### Minimal Change
Just replace the constant:
```rust
// In panic handler
std::process::exit(1); // 1 is the standard failure code
```

### Remove libc Dependency
If libc is only used for EXIT_FAILURE:
1. Remove from Cargo.toml
2. Use standard exit codes (0 for success, 1 for failure)

### Full Modernization
Consider using Result-based main:
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... existing code
}
```

## Testing Requirements
- Verify panic handler still works correctly
- Check exit code is properly set on panic
- Ensure no other libc usage is broken
- Test on different platforms if removing libc

## Success Criteria
- No direct libc usage for exit codes
- Panic handler still exits with non-zero code
- Code is more idiomatic Rust
- Potentially one less dependency

## Benefits
1. **Portability**: Standard library is more portable than libc
2. **Idiomaticity**: Follows Rust best practices
3. **Simplicity**: One less C dependency
4. **Type Safety**: If using ExitCode type

## Example Test Script
```bash
#!/bin/bash
# Test that panic exits with failure code
cargo build
./target/debug/aether --simulate-panic || echo "Exit code: $?"
# Should print "Exit code: 1"
```

## Estimated Effort
15-30 minutes

## Dependencies
- Check if libc is used elsewhere before removing
- Coordinate with any other error handling improvements

## Notes
- This is a minor improvement but makes code more Rust-idiomatic
- Good task for someone learning Rust idioms
- Could be combined with other error handling improvements
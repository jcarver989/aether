# Task: Fix Input Component Method Visibility and Call Patterns

## Priority: Medium

## Overview
While the InputState methods in the Input component are correctly private, they're being called directly from `handle_key_event()` instead of through the Action system. This is related to Task #1 but focuses on the architectural pattern of method visibility and call chains.

## Current State
In `src/components/input.rs`:
- InputState methods (`insert_char`, `delete_char`, etc.) are private ✓
- These methods are called from `handle_key_event()` ✗
- State mutations happen outside the `update()` method ✗

## Expected Behavior
Following the Action pattern:
1. Private state mutation methods should ONLY be called from `update()`
2. No component methods should be public except those in the Component trait
3. State changes should have a clear audit trail through actions

## Implementation Steps

This task should be done in conjunction with Task #1. The focus here is on ensuring:

1. **Verify method visibility**:
   ```rust
   // These should remain private
   impl InputState {
       fn insert_char(&mut self, ch: char) { ... }
       fn delete_char(&mut self) { ... }
       // etc.
   }
   ```

2. **Create a clear call hierarchy**:
   ```
   External Event
   ↓
   handle_key_event() → returns Action
   ↓
   update() → calls private mutation methods
   ↓
   Private methods mutate state
   ```

3. **Document the pattern**:
   ```rust
   impl Input {
       // ONLY called from update() when handling actions
       fn apply_insert_char(&mut self, ch: char) {
           self.state.insert_char(ch);
       }
   }
   ```

## Code Organization Pattern

```rust
impl Component for Input {
    // Public interface - receives events, returns actions
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        // ONLY return actions, no state mutation
    }
    
    // Public interface - receives actions, mutates state
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::InsertChar(ch) => {
                self.apply_insert_char(ch); // Call private method
            }
            // etc.
        }
        Ok(None)
    }
}

impl Input {
    // Private helper methods - ONLY called from update()
    fn apply_insert_char(&mut self, ch: char) {
        self.state.insert_char(ch);
    }
}
```

## Testing Requirements
- Add tests that verify methods cannot be called externally
- Test that state changes only happen through actions
- Verify the component's public API is minimal
- Add documentation tests showing proper usage

## Success Criteria
- All state mutation happens through private methods
- Private methods are only called from `update()`
- Clear documentation of the call hierarchy
- Component follows encapsulation principles

## Benefits
1. **Encapsulation**: Internal implementation details are hidden
2. **Maintainability**: Clear boundaries between public API and implementation
3. **Debugging**: All state changes traceable through actions
4. **Consistency**: All components follow the same pattern

## Relationship to Other Tasks
- Must be done with Task #1 (Fix Action Pattern Violations)
- Sets pattern for other components to follow
- Supports the architecture defined in CLAUDE.md

## Estimated Effort
1 hour (when done with Task #1)

## Notes
- This is about establishing good architectural patterns
- The pattern should be documented for other developers
- Consider adding architectural tests to enforce patterns
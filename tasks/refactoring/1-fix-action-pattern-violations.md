# Task: Fix Action Pattern Violations in Input Component

## Priority: Critical

## Overview
The `Input` component violates the core Action pattern by directly mutating state in `handle_key_event()` instead of emitting actions for state changes. This violates the architectural principles outlined in CLAUDE.md.

## Current Behavior
In `src/components/input.rs`, the `handle_key_event()` method directly calls state mutation methods:
- Lines 243: `self.state.insert_char(c)`
- Lines 247: `self.state.insert_newline()`
- Lines 260: `self.state.delete_char()`
- Lines 264: `self.state.move_cursor_left()`
- Lines 268: `self.state.move_cursor_right()`
- Lines 272: `self.state.move_cursor_up()`
- Lines 276: `self.state.move_cursor_down()`

## Expected Behavior
All state mutations should happen through the Action system:
1. `handle_key_event()` should emit actions like `Action::InsertChar(char)`, `Action::MoveCursor(Direction)`, etc.
2. The `update()` method should handle these actions and call the private state mutation methods
3. No public methods should directly mutate component state

## Implementation Steps

1. **Define new Input-specific actions** in `src/action.rs`:
   ```rust
   // Add to the Action enum
   InsertChar(char),
   InsertNewline,
   DeleteChar,
   MoveCursor(CursorDirection),
   ```

2. **Create CursorDirection enum** in `src/action.rs`:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
   pub enum CursorDirection {
       Left,
       Right,
       Up,
       Down,
   }
   ```

3. **Refactor handle_key_event()** to emit actions instead of mutating state:
   ```rust
   KeyCode::Char(c) => Ok(Some(Action::InsertChar(c))),
   KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
       Ok(Some(Action::InsertNewline))
   }
   // etc.
   ```

4. **Update the update() method** to handle the new actions:
   ```rust
   match action {
       Action::InsertChar(c) => {
           self.state.insert_char(c);
       }
       Action::InsertNewline => {
           self.state.insert_newline();
       }
       // etc.
   }
   ```

5. **Ensure InputState methods remain private** - they should only be called from within the `update()` method

## Testing Requirements
- Verify all user input still works correctly (typing, cursor movement, deletion)
- Ensure actions are properly emitted and can be traced/logged
- Test that the component state cannot be mutated from outside the component
- Add unit tests that verify actions are emitted correctly from `handle_key_event()`

## Success Criteria
- No direct state mutations in `handle_key_event()`
- All state changes go through the Action system
- Component follows the Action pattern as defined in CLAUDE.md
- All existing functionality remains intact

## References
- CLAUDE.md: Action Pattern Implementation section
- Related components that properly implement the pattern: `chat.rs`, `tool_call.rs`

## Estimated Effort
2-3 hours

## Dependencies
None - this can be implemented independently
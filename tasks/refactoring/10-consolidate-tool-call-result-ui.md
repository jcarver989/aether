# Task: Consolidate Tool Call and Result UI Rendering

## Overview
Currently, tool calls and their results are rendered as separate UI blocks. This creates visual fragmentation where a tool call appears in one block and its result appears in another block below it. We should consolidate these into a single unified block for better UX.

## Current State
- `ContentBlock::ToolCallBlock` - renders tool call with icon, name, params
- `ContentBlock::ToolResultBlock` - renders result content separately 
- Both have their own borders and appear as distinct blocks
- Tool results reference calls via `tool_call_id` string matching

## Desired End State
Users should see:
```
┌─────────────────────────────────────┐
│ 🔄 read_file(/path/to/file.txt)     │
│ File contents:                      │
│ Hello world                         │ 
│ This is line 2                      │
└─────────────────────────────────────┘
```

Instead of:
```
┌─────────────────────────────────────┐
│ 🔄 Tool Call: read_file             │
│ read_file(/path/to/file.txt)        │
└─────────────────────────────────────┘
┌─────────────────────────────────────┐
│ Result (id123)                      │
│ File contents:                      │
│ Hello world                         │
│ This is line 2                      │
└─────────────────────────────────────┘
```

## Implementation Plan

### 1. Data Structure Changes
Create new enum variant in `ContentBlock`:
```rust
ToolCallWithResult {
    id: String,
    name: String,
    params: String,
    result: Option<String>,
    timestamp: DateTime<Utc>,
    state: ToolCallState,
    expanded: bool,
}
```

### 2. Rendering Updates
- Modify `render_tool_call_with_result()` method
- Line 1: `<state_icon> <tool_name>(<params>)`
- Lines 2-N: Result content (if `result.is_some()`)
- Single bordered block encompassing both
- Maintain expansion/collapse functionality for results

### 3. Message Conversion Logic
Update `From<&ChatMessage>` implementation:
- Track pending tool calls in a temporary state
- When tool result arrives, find matching call and merge
- Handle edge cases (result before call, missing calls/results)
- May need intermediate storage structure during conversion

### 4. Action System Updates
- Update actions that operate on tool calls/results
- `Action::ToggleToolCall` should work on merged blocks
- `Action::UpdateToolCallState` needs to handle merged structure

### 5. State Management
- Ensure proper pairing of calls with results
- Handle async arrival order (calls vs results)
- Maintain backward compatibility during transition

## Technical Considerations

### Complexity: Medium
- Rendering changes are straightforward
- State management is the main complexity
- Need careful handling of async tool call/result pairing

### Risk Areas
- Pairing logic must be robust across different LLM response patterns
- Ensure no tool calls or results get lost during conversion
- Action routing needs to work correctly with new block type

### Testing Strategy
- Unit tests for tool call/result pairing logic
- UI tests for rendering of merged blocks
- Integration tests with various tool call scenarios
- Test edge cases (missing results, duplicate IDs, etc.)

## Dependencies
- No external dependencies
- Requires understanding of current Action pattern
- Should maintain theme consistency

## Acceptance Criteria
1. Tool calls and results appear in single unified blocks
2. All existing functionality preserved (expand/collapse, state updates)
3. Proper handling of edge cases (missing results, async arrival)
4. No visual regressions in other UI components
5. Action system continues to work correctly
6. Tests pass and new test coverage added

## Estimated Effort
2-3 days for experienced Rust/Ratatui developer

## Priority
Medium - Nice UX improvement but not blocking core functionality
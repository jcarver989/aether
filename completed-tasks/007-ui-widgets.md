# Task 007: UI Widgets Implementation

## Objective
Create specialized UI widgets for chat display, tool calls, and user input.

## Requirements
1. In `src/ui/widgets/chat.rs`, implement:
   - Chat message widget with proper formatting
   - Support for different message types (user, assistant, tool)
   - Code block rendering with syntax awareness
   - Markdown-style formatting for assistant messages

2. In `src/ui/widgets/tool_call.rs`, implement:
   - Visual representation of tool invocations
   - Collapsible tool parameters display
   - Tool result formatting
   - Progress indicator for running tools

3. In `src/ui/widgets/input.rs`, implement:
   - Multi-line input widget
   - Basic text editing (insert, delete, backspace)
   - Cursor movement
   - Submit on Enter (with Shift+Enter for newline)

## Deliverables
- Complete chat widget with message rendering
- Tool call visualization widget
- Input widget with editing capabilities
- Consistent styling across all widgets
- Example usage in main app

## Notes
- Use Ratatui's styling consistently
- Handle text wrapping for long messages
- Consider accessibility (clear visual hierarchy)
- Keep tool results readable but compact
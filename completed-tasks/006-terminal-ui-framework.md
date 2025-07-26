# Task 006: Terminal UI Framework

## Objective
Implement the core terminal UI framework using Ratatui with basic layout and event handling.

## Requirements
1. In `src/ui/app.rs`, implement:
   - Main application state structure
   - Layout with chat area and input area
   - Basic rendering loop
   - State management for UI components

2. In `src/ui/event.rs`, implement:
   - Keyboard event handling
   - Support for Ctrl+C to exit
   - Text input handling
   - Scrolling support for chat view

3. Create the following types:
   ```rust
   pub struct App {
       pub messages: Vec<UiMessage>,
       pub input: String,
       pub scroll_offset: u16,
       pub is_running: bool,
   }
   
   pub enum UiMessage {
       User { content: String },
       Assistant { content: String },
       ToolCall { name: String, params: String },
       ToolResult { content: String },
       Error { message: String },
   }
   ```

## Deliverables
- Basic Ratatui application structure
- Event loop with proper terminal handling
- Layout with scrollable chat area
- Input field with basic text editing
- Clean shutdown on Ctrl+C

## Notes
- Use crossterm for terminal manipulation
- Implement proper panic handling to restore terminal
- Consider viewport management for long conversations
- Keep UI responsive during LLM operations
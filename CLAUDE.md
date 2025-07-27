# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Aether is a terminal-based AI coding assistant written in Rust that provides Claude Code-like functionality through a modular architecture. It leverages the Model Context Protocol (MCP) for dynamic tool discovery and integration, supporting both OpenRouter and Ollama as LLM providers.

## Build and Development Commands

```bash
# Build the project
cargo build

# Run the project
cargo run

# Run tests
cargo test

# Run with release optimizations
cargo build --release
cargo run --release

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Architecture

The codebase follows a modular architecture with clear separation of concerns:

- **Components** (`src/components/`): Ratatui-based UI components following the Component trait
  - `chat.rs`: Chat message display with markdown formatting and scrolling
  - `input.rs`: Multi-line text input with cursor management
  - `tool_call.rs`: Interactive tool call display with expand/collapse
  - `fps.rs`: FPS counter for performance monitoring
  - `home.rs`: Main home component

- **Actions** (`src/action.rs`): Command pattern implementation for all state changes
- **Types** (`src/types.rs`): Shared data structures (ChatMessage, ToolCall, ToolCallState)
- **Configuration** (`src/config.rs`): Handles configuration and key bindings
- **TUI** (`src/tui.rs`): Terminal UI framework integration

## CRITICAL: Action Pattern Implementation

The codebase uses the Action pattern (Command pattern) for all state management. This is ESSENTIAL for maintainability, testability, and consistency.

### Action Pattern Rules (MUST FOLLOW):

1. **ALL state changes MUST go through actions**
   - Never expose public methods that directly mutate component state
   - State mutation methods should be private (`fn` not `pub fn`)
   - Only the `update(&mut self, action: Action)` method should change state

2. **Components should emit actions, not mutate external state**
   - Use `handle_key_event()` to convert user input to actions
   - Return `Ok(Some(Action::...))` to emit actions to the application
   - Let the application route actions to appropriate components

3. **Domain-specific actions are better than generic ones**
   - ✅ `Action::SubmitMessage(String)` - clear intent
   - ✅ `Action::ScrollChat(ScrollDirection)` - specific behavior
   - ❌ `Action::UpdateState(GenericState)` - too vague

4. **Action enum should include payload data when needed**
   ```rust
   pub enum Action {
       // Simple actions
       Quit,
       ClearChat,
       
       // Actions with data
       SubmitMessage(String),
       AddChatMessage(ChatMessage),
       UpdateToolCallState { id: String, state: ToolCallState },
   }
   ```

5. **Components must implement the Component trait properly**
   ```rust
   impl Component for MyComponent {
       // Convert events to actions
       fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
           match key.code {
               KeyCode::Enter => Ok(Some(Action::DoSomething)),
               _ => Ok(None),
           }
       }
       
       // Handle actions and update state
       fn update(&mut self, action: Action) -> Result<Option<Action>> {
           match action {
               Action::DoSomething => {
                   self.private_state_change(); // Only private methods!
               }
               _ => {}
           }
           Ok(None)
       }
   }
   ```

### Common Action Pattern Mistakes to Avoid:

- ❌ Exposing `pub fn set_something(&mut self)` methods
- ❌ Directly calling component methods from other components  
- ❌ Mutating state outside of the `update()` method
- ❌ Using generic actions instead of domain-specific ones
- ❌ Forgetting to add required derives to types used in actions

### Benefits of Proper Action Implementation:

- **Testability**: Can test state changes by sending actions
- **Traceability**: All state changes go through one pathway
- **Consistency**: Uniform event handling across components
- **Debugging**: Easy to log/trace all state mutations
- **Extensibility**: Easy to add new behaviors via actions

## Key Design Principles

1. **Action-Driven State Management**: All state changes go through the Action system
2. **Component Isolation**: Components only communicate via actions
3. **Tool Agnostic**: All tools are provided via MCP servers
4. **Provider Flexible**: Supports multiple LLM providers through a common interface

## Development Notes

- Uses Tokio for async runtime
- Ratatui for terminal UI with Component trait pattern
- All components must implement proper action handling
- Types used in actions need `PartialEq`, `Eq`, `Serialize`, `Deserialize` derives

### Testing

1. Test components by sending actions to `update()` method
2. Verify components emit correct actions from `handle_key_event()`
3. Tests should be placed in a `tests/` directory
4. Use "real" implementations where possible, avoid mocks
5. Create in-memory Fake implementations when needed
6. Tests should be thread safe and self-contained
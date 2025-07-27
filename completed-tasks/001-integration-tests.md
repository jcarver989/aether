# Task 001: Integration Tests

## Overview
Create comprehensive integration tests for the aether application, following the patterns established in the v0 implementation.

## Missing Components
The new app lacks the `tests/` directory that was present in v0, which contained:
- `llm_provider.rs` - Tests for LLM provider implementations
- `mcp_integration.rs` - Tests for MCP client functionality  
- `streaming_tests.rs` - Tests for streaming response handling
- `tool_registry.rs` - Tests for tool registry and discovery

## Requirements

### Test Structure
- Create `tests/` directory in project root
- Follow Rust integration test conventions
- Use "real" implementations where possible, avoid mocks
- Create in-memory Fake implementations when needed
- Tests should be thread-safe and self-contained

### Test Files to Create

#### 1. `tests/llm_provider.rs`
- Test OpenRouter provider initialization and requests
- Test Ollama provider initialization and requests
- Test error handling for invalid API keys/endpoints
- Test streaming response parsing
- Test tool call generation and parsing

#### 2. `tests/mcp_integration.rs`  
- Test MCP client connection to servers
- Test tool discovery and registration
- Test tool execution and response handling
- Test error handling for failed connections
- Test configuration loading from mcp.json

#### 3. `tests/streaming_tests.rs`
- Test streaming response handling in UI
- Test partial message updates
- Test stream interruption and recovery
- Test concurrent streaming operations

#### 4. `tests/tool_registry.rs`
- Test tool registration and lookup
- Test tool parameter validation
- Test tool execution workflow
- Test tool result formatting

### Test Utilities
- Create helper functions for test setup/teardown
- Mock MCP servers for testing
- Test data generators for messages and tool calls
- Assertion helpers for UI state validation

## Implementation Notes
- Tests inherit all best practices from "normal code" (DRY, helper methods, etc.)
- Ensure all tests pass before moving to other tasks
- Tests should validate both happy path and error scenarios
- Use `tempfile` crate for temporary test files/directories

## Acceptance Criteria
- [ ] All 4 test files created and compiling
- [ ] Tests cover core functionality from v0 app
- [ ] All tests pass with `cargo test`
- [ ] Test coverage includes error scenarios
- [ ] Helper utilities reduce test verbosity
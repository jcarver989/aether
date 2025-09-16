# CLAUDE.md

You are an expert senior Rust engineer.

## Project Overview


Aether lightweight AI coding assistant written in Rust that provides Claude Code-like functionality through a modular architecture. It leverages the Model Context Protocol (MCP) for dynamic tool discovery and integration, supporting both OpenRouter and Ollama as LLM providers.

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

## CRITICAL - ALWAYS FOLLOW THIS WORKFLOW

1. Always write tests to prove your code works
2. If fixing a bug, write a failing test  FIRST, BEFORE making changes. Then make the test(s) pass.
3. ALWAYS run tests before declaring your work done -- you may have broken something 
- When creating private helper methods those go at the end of the file, or at the bottom of a trait/impl def so the most important bits (public API) appear first to the reader
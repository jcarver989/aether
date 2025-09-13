# Wisp AI Assistant

Wisp is an AI-powered coding assistant designed for Rust developers. It provides intelligent code analysis, generation, and assistance using local language models.

## Features

- **Rust-focused AI assistance**: Specialized for Rust systems programming and async/await patterns
- **Local LLM support**: Runs entirely locally without internet connectivity requirements
- **Interactive coding interface**: Real-time responses with tool execution visualization
- **Agent-based architecture**: Built on the aether framework for agent-based interactions
- **Rich terminal UI**: Colorful, informative terminal output with progress indicators

## Installation

Wisp is built as a Rust crate and requires Cargo to build. To install:

```bash
# Clone the repository
 git clone <repository-url>
 cd wisp

# Build the project
 cargo build --release
```

## Usage

To use Wisp, run it with a coding question or request:

```bash
./wisp "implement a binary search tree in Rust"
```

Or with a more complex request:

```bash
./wisp "explain how async/await works in Rust and show examples"
```

## Configuration

Wisp looks for an `AGENTS.md` file in the current directory to provide additional context and instructions to the AI agent. This file contains guidelines for the AI's behavior and capabilities.

## Dependencies

Wisp depends on:

- Local LLM (via llama.cpp)
- aether framework
- mcp-lexicon
- tokio with full features
- Various Rust ecosystem crates for terminal UI and utilities

## Development

To contribute to Wisp:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests with `cargo test`
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.
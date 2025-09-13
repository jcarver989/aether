# Wisp

Wisp is an Ethereal AI Assistant designed to help with Rust development tasks. It provides a terminal-based interface for interacting with AI agents that specialize in code analysis, generation, and modification.

## Features

- Terminal-based interface with colorful output
- Integration with local LLM models via llama.cpp
- Tool calling capabilities for code analysis and manipulation
- Support for custom agent instructions via AGENTS.md
- Progress indicators and tool execution feedback

## Installation

Wisp is built as a Rust crate and requires a Rust toolchain to compile:

```bash
# Clone the repository
 git clone <repository-url>
 cd wisp
 
# Build the project
 cargo build --release
```

## Usage

Run Wisp with a coding question or request:

```bash
./wisp "Implement a binary search tree in Rust"
```

Or with a quoted prompt:

```bash
./wisp "help me implement a binary search tree"
```

## AGENTS.md

Wisp can load custom agent instructions from an `AGENTS.md` file in the current directory. This file defines the behavior and capabilities of the AI agent. If no AGENTS.md is found, Wisp will use default instructions.

## Dependencies

Wisp depends on several crates including:

- `aether` - Core agent framework
- `mcp_lexicon` - Language model communication protocols
- `tokio` - Asynchronous runtime
- `indicatif` - Progress bars and spinners
- `owo-colors` - Colorful terminal output

## License

This project is licensed under the MIT License.
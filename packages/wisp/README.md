# Wisp

Wisp is a Terminal User Interface (TUI) application designed to provide an ethereal AI assistant experience. It leverages the Aether framework to deliver powerful AI capabilities with a clean, colorful terminal interface.

## Features

- **Colorful TUI**: Beautiful terminal interface with custom color schemes
- **AI Assistant**: Powered by Aether's agent system and local LLM capabilities
- **Tool Integration**: Supports tool calling for extended functionality
- **Agent Management**: Loads agents from AGENTS.md file for custom behavior
- **Real-time Response**: Streaming responses with progress indicators

## Installation

To build and install Wisp, you'll need Rust installed on your system. Then run:

```bash
 cargo install --path .
```

## Usage

Wisp can be invoked with a prompt to interact with the AI assistant:

```bash
wisp "Implement a binary search tree in Rust"
```

Or for a more complex interaction:

```bash
wisp "Explain how async/await works in Rust and show an example"
```

## AGENTS.md Integration

Wisp can load agents from an `AGENTS.md` file in the current directory. This file defines custom agent behavior and capabilities, which will be used as a system prompt for the AI assistant.

## Requirements

- Rust 1.70 or later
- A local LLM model (configured via aether)

## License

This project is licensed under the MIT License - see the LICENSE file for details.

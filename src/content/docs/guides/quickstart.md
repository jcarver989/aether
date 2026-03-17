---
title: Quick Start
description: Get Aether running locally in minutes.
---

## Prerequisites

- Rust 1.80+ (install via [rustup](https://rustup.rs))
- An API key for at least one LLM provider (Anthropic, OpenRouter, etc.)

## Clone and build

```bash
git clone https://github.com/joshka/aether.git
cd aether
cargo build
```

## Run the TUI

```bash
cargo run --bin wisp
```

## Run headless

```bash
cargo run --bin aether -- --headless "Your prompt here"
```

## Configuration

Aether reads MCP server configuration from `.mcp.json` in your project root. See the [Introduction](/guides/introduction/) for an overview of the architecture.

---
title: Introduction
description: What is Aether and why use it?
---

Aether is a modular, open-source AI agent framework written in Rust. It provides the building blocks for creating AI-powered coding assistants and autonomous agents.

## Why Aether?

- **Modular architecture** — Use only the packages you need. Swap providers, tools, and interfaces independently.
- **Built on MCP** — The Model Context Protocol gives your agents dynamic tool discovery and integration.
- **Multiple providers** — Anthropic, OpenRouter, Bedrock, Ollama, OpenAI, and more through a unified interface.
- **Rich TUI** — A terminal interface with markdown rendering, syntax highlighting, and inline diffs.
- **Evaluation framework** — Test agent behavior with Crucible's assertion-based eval runner.

## Architecture

Aether is organized as a Cargo workspace with focused packages:

| Package | Purpose |
|---------|---------|
| `aether-core` | Agent runtime, context management, conversation loop |
| `llm` | Unified LLM provider interface |
| `mcp-servers` | Built-in MCP tool servers (file I/O, bash, web, LSP) |
| `wisp` | Terminal UI application |
| `crucible` | Evaluation and testing framework |
| `aether-cli` | Command-line interface |
| `tui` | Low-level TUI rendering primitives |
| `mcp-utils` | MCP client utilities |

## Next steps

Head to the [Quick Start](/guides/quickstart/) guide to get Aether running locally.

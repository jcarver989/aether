# Aether

[![CI](https://github.com/jcarver989/aether/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jcarver989/aether/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/aether-agent-cli.svg)](https://crates.io/crates/aether-agent-cli)
[![Rust](https://img.shields.io/badge/Made_with-Rust-orange.svg)](https://www.rust-lang.org)

Aether is a modular toolkit for building AI agents (LLM + prompt + tools + loop), written in Rust. It gives _you_ full control over every token in context -- no injected prompts, tools or telemetry.

You can use Aether as:

1. A batteries-included coding agent that runs in a TUI, IDE/Editor (via [ACP](https://zed.dev/acp)) or headless.
2. As a library, where you can assemble the components you want a la carte.

**[Documentation](https://aether-agent.io)** · **[GitHub](https://github.com/jcarver989/aether)**

![Aether demo](demo.gif)

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

- [Why Aether?](#why-aether)
- [Quick Start](#quick-start)
  - [1. **Install**](#1-install)
  - [2. **Create your first agent**](#2-create-your-first-agent)
  - [3. **Run it**](#3-run-it)
- [Using Aether as a library](#using-aether-as-a-library)
- [Packages](#packages)
  - [Binary distribution (maintainers)](#binary-distribution-maintainers)
- [License](#license)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

## Why Aether?

1. **Your context** — Agents default to an empty system prompt with no tools, so _you_ control _every_ token in the agent's context window. Inspect the fully-rendered prompt anytime with `aether show-prompt`.
2. **Your model** — Use any LLM you want -- Anthropic, OpenAI, OpenRouter, DeepSeek, Gemini, Moonshot, ZAI, Llama.cpp, and Ollama are [supported out of the box](https://aether-agent.io/aether/configuration/llm-providers/). Implement your own via the `StreamingModelProvider` trait, or [alloy models together](https://aether-agent.io/aether/configuration/llm-providers/#alloying) to combine their strenghts.
3. **Your tools** — Aether agents get tools exclusively via [MCP](https://modelcontextprotocol.io/) servers. Thus you can extend them using _any_ language, configure them using standard `mcp.json` files, and swap toolsets without touching agent code.
4. **Your interface** — Aether agents come out of the box ready to run wherever you need them to -- headlessly, in the terminal (via a TUI), in an editor (via ACP integration), or as a Rust library.

## Quick Start
Create a custom agent in ~5 minutes, no Rust code required.


### 1. **Install**

   **macOS** (Apple Silicon):

   ```bash
   brew install jcarver989/tap/aether
   ```

   **macOS / Linux** (x64, ARM64):

   ```bash
   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jcarver989/aether/releases/latest/download/aether-agent-cli-installer.sh | sh
   ```

   **Any platform** (requires **Rust 1.85+**):

   ```bash
   cargo install aether-agent-cli
   ```

### 2. **Create your first agent**

   ```bash
   cd your-project
   aether agent new
   ```

   ```
   ✓ Created .aether/settings.json   — agent definitions (model, prompts, tools)
   ✓ Created .aether/mcp.json        — MCP server config
   ✓ Created .aether/SYSTEM.md       — base system prompt
   ✓ Created AGENTS.md               — project-level instructions
   ```
   
   The tui will walk you through:
   
   1. Choosing an LLM. See [LLM Providers](https://aether-agent.io/aether/configuration/llm-providers/) for all supported providers and credential setup.
   2. Adding tools. Aether ships with several optional [MCP](https://modelcontextprotocol.io/) servers:
      - **Coding** — file ops, bash, grep, web fetch/search
      - **LSP** — language-aware symbol lookup, diagnostics, rename
      - **Skills** — load reusable skill files from `skills/`
      - **Tasks** — structured task management for multi-step work
      - **Sub-agents** — spawn child agents defined in `.aether/settings.json`
      - **Survey** — human-in-the-loop elicitation (ask the user questions)
  
   You can also plug in any external MCP server (stdio, SSE, or streamable HTTP). See [MCP Servers](https://aether-agent.io/aether/configuration/mcp-servers/).

### 3. **Run it**

   - In a **TUI** — interactive terminal UI:

     ```bash
     aether
     ```
   
  - In an **editor/IDE** via [ACP](https://agentclientprotocol.com/get-started/introduction):

     ```bash
     aether acp
     ```

   - As a **headless** agent:

     ```bash
     aether headless "Refactor auth module"
     ```



## Using Aether as a library

Use `aether-agent-core` as a Rust library to build your own agent in ~25 lines. Bring your own model via the `StreamingModelProvider` trait, or alloy models together to round-robin across providers per turn.

1. **Add dependencies**

   ```bash
   cargo add aether-agent-core tokio
   ```

2. **Write your agent**

   ```rust
   use aether_core::{
       core::{Prompt, agent},
       events::{AgentMessage, UserMessage},
       mcp::{McpSpawnResult, mcp},
   };
   use llm::providers::anthropic::AnthropicProvider;
   use std::io::{self, Write};

   #[tokio::main]
   async fn main() -> Result<(), Box<dyn std::error::Error>> {
       // 1. Create a provider (reads ANTHROPIC_API_KEY from env)
       let llm = AnthropicProvider::new(None)?;

       // 2. Spawn MCP tool servers from one or more mcp.json files
       let McpSpawnResult { tool_definitions: tools, command_tx: mcp_tx, .. } =
           mcp().from_json_files(&["mcp.json"]).await?.spawn().await?;

       // 3. Build and spawn the agent
       let (tx, mut rx, _handle) = agent(llm)
           .system_prompt(Prompt::from_globs(vec!["AGENTS.md".into()], ".".into()))
           .tools(mcp_tx, tools)
           .spawn()
           .await?;

       // 4. Send a message and stream the response
       tx.send(UserMessage::text("Hello!")).await?;

       loop {
           match rx.recv().await {
               Some(AgentMessage::Text { chunk, is_complete, .. }) => {
                   if !is_complete { print!("{chunk}"); io::stdout().flush()?; }
               }
               Some(AgentMessage::Done) => break,
               Some(AgentMessage::Error { message }) => { eprintln!("Error: {message}"); break; }
               _ => {}
           }
       }
       Ok(())
   }
   ```

## Packages

| Package | Description |
|---------|-------------|
| [`aether-agent-core`](packages/aether-core) | Core agent library — LLM + prompt + tools in a loop ([docs](https://aether-agent.io/libraries/aether-core/agent-builder/)) |
| [`llm`](packages/llm) | Multi-provider LLM abstraction ([docs](https://aether-agent.io/libraries/llm/provider-interface/)) |
| [`wisp`](packages/wisp) | Terminal UI for AI agents, built on ACP ([docs](https://aether-agent.io/aether/terminal/overview/)) |
| [`aether-agent-cli`](packages/aether-cli) | Headless CLI and ACP server for editor integration ([docs](https://aether-agent.io/aether/running/headless/)) |
| [`mcp-servers`](packages/mcp-servers) | Pre-built MCP tool servers (coding, LSP, skills, tasks, sub-agents, survey) ([docs](https://aether-agent.io/aether/built-in-servers/coding/)) |
| [`crucible`](packages/crucible) | Automated testing (evals) for LLM agents ([docs](https://aether-agent.io/libraries/crucible/evals/)) |
| [`aether-lspd`](packages/aether-lspd) | LSP daemon — shares language servers across agents |
| [`aether-project`](packages/aether-project) | Project configuration and agent catalog from `.aether/settings.json` |


### Binary distribution (maintainers)

Aether releases are built with [cargo-dist](https://github.com/axodotdev/cargo-dist) via GitHub Actions.

- Preview release artifacts locally: `dist plan`
- Build local distributable artifacts: `dist build`
- Cutting a release is tag-driven (`vX.Y.Z`) and publishes GitHub Release artifacts.

## License

MIT

# Aether

A modular toolkit for building AI agents (LLM + prompt + tools + loop), written in Rust. 

Use Aether as a library and select components a la carte, or use it to run a fully batteries-included agent that has filesystem tools, LSP server integration, sub-agents, skills and more.

![Aether demo](demo.gif)

## Why Aether?

1. **Your context** — Agents default to an empty system prompt with no tools, so _you_ control _every_ token in the agent's context window.
2. **Your model** — Use any LLM you want -- OpenAI, OpenRouter, Google, DeepSeek, Moonshot, Zai, Llama.cpp, and Ollama providers are supported out of the box. And you can implement your own provider via the `StreamableModelProvider` trait.
3. **Your tools** — Aether agents get tools exclusively via [MCP](https://modelcontextprotocol.io/) servers. Thus you can extend them using _any_ language, configure them using standard `mcp.json` files, and swap toolsets without touching agent code.
4. **Your interface** — Aether agents come out of the box ready to run wherever you need them to -- headlessly, in the terminal (via a TUI), in an editor (via ACP integration), or as a Rust library.

## Quick Start
Create a custom agent in ~10 minutes, no Rust code required.

### 1. Run a custom agent

1. **Pick a model** — Anthropic, OpenAI, OpenRouter, Google, DeepSeek, Moonshot, Zai, Llama.cpp, and Ollama are supported out of the box. Set the relevant API key:

   ```bash
   export ANTHROPIC_API_KEY=sk-ant-...
   # or OPENROUTER_API_KEY, OPENAI_API_KEY, etc. — or use Ollama for fully local
   ```

2. **Add a system prompt** — create an `AGENTS.md` file in your project root. Aether loads it automatically.

3. **Add tools** — agents get tools exclusively via [MCP](https://modelcontextprotocol.io/) servers. Create an `mcp.json` in your project root. These built-in servers are available:

   - **Coding** — file ops, bash, grep, LSP, web fetch/search
   - **Skills** — load reusable skill files from `skills/`
   - **Tasks** — structured task management for multi-step work
   - **Sub-agents** — spawn child agents defined in `.aether/settings.json`
   - **Survey** — human-in-the-loop elicitation (ask the user questions)

   ```json
   {
     "servers": {
       "coding": { "type": "in-memory" },
       "skills": { "type": "in-memory" },
       "tasks": { "type": "in-memory" },
       "subagents": { "type": "in-memory" },
       "survey": { "type": "in-memory" }
     }
   }
   ```

   You can add external MCP servers alongside the built-ins:

   ```json
   {
     "servers": {
       "coding": { "type": "in-memory" },
       "skills": { "type": "in-memory" },
       "tasks": { "type": "in-memory" },
       "subagents": { "type": "in-memory" },
       "survey": { "type": "in-memory" },
       "playwright": {
         "type": "stdio",
         "command": "npx",
         "args": ["@playwright/mcp@latest"]
       }
     }
   }
   ```

4. **Run it**

   - **TUI** — interactive terminal UI (model selected inside the TUI):

     ```bash
     cargo run -p wisp
     ```

   - **Headless CLI** — single prompt in, text out:

     ```bash
     cargo run -p aether-agent-cli --bin aether -- -m anthropic:claude-sonnet-4-5-20250929 "Refactor auth module"
     ```

   - **ACP server** — for editor/IDE integration via [ACP](https://agentclientprotocol.com/get-started/introduction):

     ```bash
     cargo run -p aether-agent-cli --bin aether-acp -- --model anthropic:claude-sonnet-4-5-20250929 --mcp-config mcp.json
     ```

### 2. Build a custom agent as a Rust library

Use `aether-agent-core` as a Rust library to build your own agent in ~25 lines. Bring your own model via the `StreamableModelProvider` trait, or alloy models together to round-robin across providers per turn.

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

       // 2. Spawn MCP tool servers from an mcp.json file
       let McpSpawnResult { tool_definitions: tools, command_tx: mcp_tx, .. } =
           mcp().from_json_file("mcp.json").await?.spawn().await?;

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

See [`examples/`](packages/aether-core/examples) for more complete examples.

## Use Cases

### Talk to local and remote LLMs

The [`llm`](packages/llm) provides a unified streaming interface for LLM providers. Anthropic, OpenRouter, OpenAI, DeepSeek, Gemini, ZAI, Ollama, and Llama.cpp are supported out of the box. If something isn't supported, you can add your own via the `StreamableModelProvider` trait. You can also alloy models together — round-robin across providers per turn to combine their strengths.

### Build a custom agent

[`aether-agent-core`](packages/aether-core) is the core crate. Create a custom agent in ~10 lines of Rust and tailor it to your domain. Aether agents start as a blank slate with no system prompt or tools, so you control every token in your agent's context window.

Agents get their tools from [MCP](https://modelcontextprotocol.io/) servers — write tool servers in any language and connect them via a standard `mcp.json` file. This repo includes several pre-built servers to get you started:

- [`mcp-servers`](packages/mcp-servers) — file operations, bash, LSP, sub-agents, tasks, and slash commands (feature-flagged)

### Run your agent in an interactive terminal

[`wisp`](packages/wisp) is a TUI (terminal UI) for Aether agents (it also works with any [ACP](https://agentclientprotocol.com/get-started/introduction) compatible agent).

### Connect your agent to an IDE or UI

[`aether-agent-cli`](packages/aether-cli) Connect your agent to any [ACP](https://agentclientprotocol.com/get-started/introduction) compatible client ([see list](https://agentclientprotocol.com/get-started/clients)).

### Run a fully-fledged, open source coding agent

Combine all the above for a "batteries-included" AI coding agent: [`wisp`](packages/wisp) (TUI) + [`aether-agent-cli`](packages/aether-cli) (ACP server) + the pre-built [MCP tool servers](packages/mcp-servers).

## Packages

| Package | Description |
|---------|-------------|
| [`aether-agent-core`](packages/aether-core) | Core agent library — LLM + prompt + tools in a loop |
| [`llm`](packages/llm) | Multi-provider LLM abstraction (Anthropic, OpenAI, OpenRouter, Ollama, etc.) |
| [`wisp`](packages/wisp) | Terminal UI for AI agents, built on ACP |
| [`aether-agent-cli`](packages/aether-cli) | Headless CLI and ACP server for editor integration |
| [`mcp-servers`](packages/mcp-servers) | Pre-built MCP tool servers (coding, skills, tasks, sub-agents, survey) |
| [`crucible`](packages/crucible) | Automated testing (evals) for LLM agents |
| [`aether-lspd`](packages/aether-lspd) | LSP daemon — shares language servers across agents |
| [`aether-project`](packages/aether-project) | Project configuration and agent catalog from `.aether/settings.json` |

## Development

Standard cargo workflow: `cargo check`, `cargo test`, `cargo fmt`, `cargo clippy`.

### Binary distribution (maintainers)

Aether releases are built with [cargo-dist](https://github.com/axodotdev/cargo-dist) via GitHub Actions.

- Preview release artifacts locally: `dist plan`
- Build local distributable artifacts: `dist build`
- Optional workflow smoke test with [act](https://github.com/nektos/act): `act pull_request -W .github/workflows/release.yml -j plan -P ubuntu-22.04=catthehacker/ubuntu:act-22.04`
- Cutting a release is tag-driven (`vX.Y.Z`) and publishes GitHub Release artifacts.
- Release artifacts include both `aether` and `aether-lspd` binaries (LSP tools depend on `aether-lspd`).
- Homebrew publishing targets `contextbridge/homebrew-tap` and requires the `HOMEBREW_TAP_TOKEN` GitHub secret.

## License

MIT

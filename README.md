# Aether

A modular Rust toolkit for building AI agents (LLM + prompt + tools + loop). Use components a la carte, or snap them together into a batteries-included coding agent. Aether has optional tools, sub-agent support, LSP integration and runs in your terminal (via a TUI), IDE/editor (via ACP), or headlessly as an async agent.

## Why Aether?

1. **Your context** — Aether agents start with no system prompt or tools; you control _every_ token in the context window.
2. **Your model** — Works with Anthropic, OpenAI, OpenRouter, Google, DeepSeek, Moonshot, Zai, Llama.cpp, or Ollama out of the box. Or, bring your own via the `StreamableModelProvider` trait.
3. **Your tools** — Aether agents get tools exclusively via [MCP](https://modelcontextprotocol.io/) servers. Thus you can extend them using _any_ language, configure them using standard `mcp.json` files, and swap toolsets without touching agent code.
4. **Your interface** — Aether agents come out of the box ready to run wherever you need them to -- headlessly, in the terminal (via a TUI), in an editor (via ACP integration), or use as a Rust library.

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
   - **Sub-agents** — spawn child agents from `sub-agents/`
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
     cargo run -p aether-cli -- -m anthropic:claude-sonnet-4-20250514 "Refactor auth module"
     ```

   - **ACP server** — for editor/IDE integration via [ACP](https://agentclientprotocol.com/get-started/introduction):

     ```bash
     cargo run -p aether-acp -- --model anthropic:claude-sonnet-4-20250514 --mcp-config mcp.json
     ```

### 2. Build a custom agent as a Rust library

Use `aether` as a Rust library to build your own agent in ~25 lines. Bring your own model via the `StreamableModelProvider` trait, or alloy models together to round-robin across providers per turn.

1. **Add dependencies**

   ```bash
   cargo add aether llm tokio
   ```

2. **Write your agent**

   ```rust
   use aether::{
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
           .system_prompt(Prompt::agents_md())  // loads AGENTS.md from cwd
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

See [`examples/`](packages/aether/examples) for more complete examples.

## Use Cases

### Talk to local and remote LLMs

The [`llm`](packages/llm) provides a unified streaming interface for LLM providers. Anthropic, OpenRouter, OpenAI, DeepSeek, Gemini, ZAI, Ollama, and Llama.cpp are supported out of the box. If something isn't supported, you can add your own via the `StreamableModelProvider` trait. You can also alloy models together — round-robin across providers per turn to combine their strengths.

### Build a custom agent

[`aether`](packages/aether) is the core crate. Create a custom agent in ~10 lines of Rust and tailor it to your domain. Aether agents start as a blank slate with no system prompt or tools, so you control every token in your agent's context window.

Agents get their tools from [MCP](https://modelcontextprotocol.io/) servers — write tool servers in any language and connect them via a standard `mcp.json` file. This repo includes several pre-built servers to get you started:

- [`mcp-servers`](packages/mcp-servers) — file operations, bash, LSP, sub-agents, tasks, and slash commands (feature-flagged)

### Run your agent in an interactive terminal

[`wisp`](packages/wisp) is a TUI (terminal UI) for Aether agents (it also works with any [ACP](https://agentclientprotocol.com/get-started/introduction) compatible agent).

### Connect your agent to an IDE or UI

[`aether-acp`](packages/aether-acp) Connect your agent to any [ACP](https://agentclientprotocol.com/get-started/introduction) compatible client ([see list](https://agentclientprotocol.com/get-started/clients)).

### Run a fully-fledged, open source coding agent

Combine all the above for a "batteries-included" AI coding agent: [`wisp`](packages/wisp) (TUI) + [`aether-acp`](packages/aether-acp) (ACP server) + the pre-built [MCP tool servers](packages/mcp-servers).

See each package's README for detailed usage: [`aether`](packages/aether), [`llm`](packages/llm), [`wisp`](packages/wisp), [`aether-acp`](packages/aether-acp).

## Development

Standard cargo workflow: `cargo check`, `cargo test`, `cargo fmt`, `cargo clippy`.

## License

MIT

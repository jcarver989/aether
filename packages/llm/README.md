# aether-llm

Multi-provider LLM abstraction layer for the Aether AI agent framework.

## Providers

| Provider | Example model string | Env var |
|----------|---------------------|---------|
| Anthropic | `anthropic:claude-sonnet-4-5-20250929` | `ANTHROPIC_API_KEY` |
| OpenAI | `openai:gpt-4o` | `OPENAI_API_KEY` |
| OpenRouter | `openrouter:moonshotai/kimi-k2` | `OPENROUTER_API_KEY` |
| ZAI | `zai:GLM-4.6` | `ZAI_API_KEY` |
| AWS Bedrock | `bedrock:us.anthropic.claude-sonnet-4-5-20250929-v1:0` | AWS credentials |
| Ollama | `ollama:llama3.2` | None (local) |
| Llama.cpp | `llamacpp` | None (local) |

## Key Types

- **`StreamingModelProvider`** -- Core trait for all LLM providers. Implement this to add a new provider.
- **`Context`** -- Manages the message history, tool definitions, and reasoning effort sent to the model.
- **`ChatMessage`** -- Message enum with variants for user, assistant, and tool call messages.
- **`ToolDefinition`** -- Describes a tool the model can invoke (name, description, JSON schema).
- **`LlmModel`** -- Catalog of known models with metadata (context window, capabilities).

## Usage

```rust,no_run
use llm::providers::openrouter::OpenRouterProvider;
use llm::StreamingModelProvider;

// Create a provider from a model string
let provider = OpenRouterProvider::default("moonshotai/kimi-k2").unwrap();
println!("Using model: {:?}", provider.model());
println!("Context window: {:?}", provider.context_window());
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `bedrock` | AWS Bedrock provider support |
| `oauth` | OAuth authentication (used by Codex provider) |
| `codex` | OpenAI Codex provider (implies `oauth`) |

## License

MIT

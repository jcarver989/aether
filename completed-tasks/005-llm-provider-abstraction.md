# Task 005: LLM Provider Abstraction

## Objective
Create an abstract interface for LLM providers with concrete implementations for OpenRouter and Ollama.

## Requirements
1. In `src/llm/provider.rs`, create provider trait:
   ```rust
   #[async_trait]
   pub trait LlmProvider {
       async fn complete(&self, request: ChatRequest) -> Result<ChatResponse>;
       async fn complete_stream(&self, request: ChatRequest) -> Result<ChatStream>;
       fn get_model(&self) -> &str;
   }
   
   pub struct ChatRequest {
       pub messages: Vec<ChatMessage>,
       pub tools: Vec<ToolDefinition>,
       pub temperature: Option<f32>,
   }
   
   pub enum ChatMessage {
       System { content: String },
       User { content: String },
       Assistant { content: String },
       Tool { tool_call_id: String, content: String },
   }
   ```

2. In `src/llm/openrouter.rs`, implement OpenRouter provider:
   - Use async-openai crate with custom base URL
   - Handle API key authentication
   - Map internal types to OpenAI API format

3. In `src/llm/ollama.rs`, implement Ollama provider:
   - Use async-openai crate with local endpoint
   - Handle Ollama-specific model formats
   - Support for local model management

## Deliverables
- Provider trait definition
- Complete OpenRouter implementation
- Complete Ollama implementation
- Factory function to create provider based on config
- Integration tests for both providers

## Notes
- Handle streaming responses properly
- Map tool definitions to OpenAI function format
- Implement proper error handling for API failures
- Consider rate limiting and retry logic for MVP
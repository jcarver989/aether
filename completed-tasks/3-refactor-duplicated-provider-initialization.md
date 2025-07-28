# Task: Refactor Duplicated Provider Initialization Logic

## Priority: High

## Overview
In `src/main.rs` lines 70-108, the provider creation and app initialization logic is duplicated between OpenRouter and Ollama providers. This violates DRY principles and makes maintenance harder.

## Current Behavior
The code has a large match statement with nearly identical logic for each provider:
- Create provider with error handling
- Create agent with provider
- Create app with agent
- Run app

This pattern repeats for both OpenRouter and Ollama with only minor differences in provider creation.

## Expected Behavior
- Extract common logic into reusable functions or traits
- Reduce code duplication
- Make it easier to add new providers in the future
- Maintain provider-specific configuration while sharing common initialization

## Implementation Steps

### Option 1: Factory Pattern (Recommended)
1. **Create a provider factory function**:
   ```rust
   fn create_provider(config: &Config) -> Result<Box<dyn LlmProvider>> {
       match config.config.llm.provider {
           ProviderType::OpenRouter => {
               let api_key = config.config.llm.openrouter_api_key.as_ref()
                   .ok_or_else(|| anyhow!("OpenRouter API key not found"))?;
               
               Ok(Box::new(OpenRouterProvider::new(
                   api_key.clone(),
                   config.config.llm.model.clone(),
               )?))
           }
           ProviderType::Ollama => {
               Ok(Box::new(OllamaProvider::new(
                   Some(config.config.llm.ollama_base_url.clone()),
                   config.config.llm.model.clone(),
               )?))
           }
       }
   }
   ```

2. **Simplify main function**:
   ```rust
   // After MCP setup...
   let provider = create_provider(&config)?;
   let agent = Agent::new(provider, tool_registry, config.config.agent_context.clone());
   let mut app = App::new(&args, agent)?;
   app.run().await?;
   ```

### Option 2: Builder Pattern
1. **Create a provider builder**:
   ```rust
   struct ProviderBuilder<'a> {
       config: &'a Config,
   }
   
   impl<'a> ProviderBuilder<'a> {
       fn build(&self) -> Result<Box<dyn LlmProvider>> {
           // Implementation here
       }
   }
   ```

### Option 3: Extension Trait
1. **Add methods to Config**:
   ```rust
   impl Config {
       fn create_provider(&self) -> Result<Box<dyn LlmProvider>> {
           // Implementation here
       }
   }
   ```

## Considerations

1. **Error Messages**: Ensure error messages remain specific and helpful
2. **Type Erasure**: Using `Box<dyn LlmProvider>` has a small performance cost but greatly improves code organization
3. **Future Providers**: Design should make it trivial to add new providers

## Testing Requirements
- Verify both providers still initialize correctly
- Test error cases (missing API keys, invalid config)
- Ensure error messages are still descriptive
- Performance should remain unchanged

## Success Criteria
- No duplicated code between provider initialization paths
- Adding a new provider requires minimal code changes
- Error handling remains robust and informative
- Code is more readable and maintainable

## Example Implementation

```rust
// Before: 40+ lines of duplicated code

// After:
async fn run_app(args: Cli, config: Config, tool_registry: ToolRegistry) -> Result<()> {
    let provider = create_provider(&config)
        .context("Failed to initialize LLM provider")?;
    
    let agent = Agent::new(
        provider,
        tool_registry,
        config.config.agent_context.clone(),
    );
    
    let mut app = App::new(&args, agent)?;
    app.run().await
}

// In main():
run_app(args, config, tool_registry).await?;
```

## Benefits
1. **Maintainability**: Single place to update provider initialization logic
2. **Testability**: Can unit test provider creation separately
3. **Extensibility**: Easy to add new providers
4. **Readability**: Main function focuses on high-level flow

## Estimated Effort
1-2 hours

## Dependencies
- None - this is a pure refactoring task
- Should not change any external interfaces
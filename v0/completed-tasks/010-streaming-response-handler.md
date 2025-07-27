# Task 010: Streaming Response Handler

## Objective
Implement proper streaming support for LLM responses to provide real-time feedback in the UI.

## Requirements
1. Enhance LLM providers to support streaming:
   - Modify provider trait to return stream of response chunks
   - Implement streaming for both OpenRouter and Ollama
   - Handle partial tool call detection in stream

2. In UI, implement streaming display:
   - Show partial responses as they arrive
   - Update UI smoothly without flickering
   - Handle backpressure appropriately
   - Indicate when response is complete

3. Stream processing pipeline:
   ```rust
   pub enum StreamChunk {
       Content(String),
       ToolCallStart { id: String, name: String },
       ToolCallArgument { id: String, argument: String },
       ToolCallComplete { id: String },
       Done,
   }
   ```

## Deliverables
- Streaming support in LLM providers
- Real-time UI updates for responses
- Proper handling of tool calls in streams
- Smooth user experience without lag
- Error handling for stream interruptions

## Notes
- Use tokio streams effectively
- Consider buffering for UI updates
- Handle network interruptions gracefully
- Test with slow connections
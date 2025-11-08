# Type Conversions

**Location**: `packages/aether/src/llm/tools.rs`

## Pattern: TryFrom for Fallible Conversions

```rust
impl TryFrom<&ToolCallRequest> for CallToolRequestParam {
    type Error = ToolCallError;

    fn try_from(request: &ToolCallRequest) -> Result<Self, Self::Error> {
        // Strip namespace prefix
        let name = request.name.strip_prefix("mcp__").unwrap_or(&request.name);

        // Validate
        if !request.arguments.is_object() {
            return Err(ToolCallError::InvalidArguments {
                tool_name: name.to_string(),
                reason: "must be object".to_string(),
            });
        }

        Ok(Self { name: name.to_string(), arguments: request.arguments.clone() })
    }
}

// Usage: let param = CallToolRequestParam::try_from(&request)?;
```

## Pattern: Tuple Inputs for Multi-Source Conversion

```rust
impl TryFrom<(&ToolCallRequest, CallToolResult)> for ToolCallResult {
    type Error = ToolCallError;

    fn try_from((req, result): (&ToolCallRequest, CallToolResult)) -> Result<Self, Self::Error> {
        Ok(Self {
            tool_use_id: req.id.clone(),  // From request
            content: result.content,       // From result
        })
    }
}

// Usage: let result = ToolCallResult::try_from((&request, call_result))?;
```

## Core Principles

- **TryFrom for fallible conversions** - Return specific error types, not String
- **From for infallible conversions** - No validation needed, always succeeds
- **Tuple inputs** - Combine data from multiple sources `(&A, B)`
- **Reference inputs** - Use `&T` when input shouldn't be consumed
- **Specific error types** - Enables pattern matching, better than String errors
- **Validation during conversion** - Fail fast with clear errors

## When to Choose

- **TryFrom**: Conversion can fail, needs validation, parsing external data
- **From**: Simple wrapping, always succeeds, no validation
- **Don't use**: When conversion requires async operations (use async methods instead)

## Why

- Type system enforces error handling with `?`
- Clear conversion logic in one place
- Self-documenting with input/output types
- Caller can pattern match on specific errors

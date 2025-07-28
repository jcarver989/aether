# Task 02: Core Hook Infrastructure

## Overview
Implement the foundational hook system including core traits, types, and basic integration with the MCP tool registry.

## Dependencies
- None (this is the foundation task)

## Deliverables

### 1. Create Hook Module Structure
Create the following file structure:
```
src/hooks/
├── mod.rs       # Public interface and re-exports
├── types.rs     # Core traits and types
└── context.rs   # Hook context implementation
```

### 2. Core Types (`src/hooks/types.rs`)

Implement the following types:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait Hook: Send + Sync {
    async fn pre_execute(&self, context: &HookContext) -> Result<HookResult>;
    async fn post_execute(&self, context: &HookContext, result: &Value) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookResult {
    pub action: HookAction,
    pub context: Option<HookResultContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookAction {
    Allow,
    Deny(String),
    RequireApproval,
    ModifyArgs(Value),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookResultContext {
    FileModification {
        path: String,
        diff: String,
        operation: FileOperation,
    },
    BatchFileModification {
        operations: Vec<FileModificationInfo>,
    },
    CommandExecution {
        command: String,
        args: Vec<String>,
        working_dir: Option<String>,
    },
    NetworkRequest {
        url: String,
        method: String,
        headers: Option<HashMap<String, String>>,
    },
    Custom(Value),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperation {
    Create,
    Modify,
    Delete,
    Rename { from: String, to: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileModificationInfo {
    pub path: String,
    pub operation: FileOperation,
    pub size_bytes: Option<u64>,
    pub preview: Option<String>,
}
```

### 3. Hook Context (`src/hooks/context.rs`)

```rust
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct HookContext {
    pub tool_name: String,
    pub server_name: String,
    pub args: Value,
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
}

impl HookContext {
    pub fn new(
        tool_name: String,
        server_name: String,
        args: Value,
        session_id: String,
    ) -> Self {
        Self {
            tool_name,
            server_name,
            args,
            timestamp: Utc::now(),
            session_id,
        }
    }
}
```

### 4. Update Action Enum
Add hook-related actions to `src/action.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    // ... existing actions ...
    
    // Hook-related actions
    PromptPermission {
        tool_name: String,
        message: String,
        context: Option<HookResultContext>,
        callback: tokio::sync::oneshot::Sender<PermissionResponse>,
    },
    DismissPermissionPrompt,
    OpenFeedbackInput,
}

#[derive(Debug)]
pub enum PermissionResponse {
    Approved,
    Denied,
    DeniedWithFeedback(String),
}
```

### 5. Integration with ToolRegistry

Modify `src/mcp/registry.rs` to add hook support:

```rust
use crate::hooks::{Hook, HookContext, HookAction};
use tokio::sync::mpsc::Sender;

pub struct ToolRegistry {
    // ... existing fields ...
    hooks: Vec<Box<dyn Hook>>,
    action_sender: Option<Sender<Action>>,
    session_id: String,
}

impl ToolRegistry {
    // Add method to register hooks
    pub fn register_hook(&mut self, hook: Box<dyn Hook>) {
        self.hooks.push(hook);
    }
    
    // Add method to set action sender
    pub fn set_action_sender(&mut self, sender: Sender<Action>) {
        self.action_sender = Some(sender);
    }
    
    // Modify invoke_tool to check hooks
    pub async fn invoke_tool(&self, tool_name: &str, args: Value) -> Result<Value> {
        // Create hook context
        let context = HookContext::new(
            tool_name.to_string(),
            self.get_server_for_tool(tool_name)?.to_string(),
            args.clone(),
            self.session_id.clone(),
        );
        
        // Run pre-execution hooks
        for hook in &self.hooks {
            let result = hook.pre_execute(&context).await?;
            match result.action {
                HookAction::Deny(reason) => {
                    return Err(color_eyre::Report::msg(format!(
                        "Tool execution denied by hook: {}", reason
                    )));
                }
                HookAction::RequireApproval => {
                    // For now, just log - full implementation in later task
                    tracing::warn!("Hook requires approval for {}", tool_name);
                }
                _ => {}
            }
        }
        
        // Execute tool (existing code)
        let result = self.mcp_client.execute_tool(
            self.get_server_for_tool(tool_name)?,
            tool_name,
            args
        ).await?;
        
        // Run post-execution hooks
        for hook in &self.hooks {
            hook.post_execute(&context, &result).await?;
        }
        
        Ok(result)
    }
}
```

## Testing Requirements

1. Unit tests for all hook types and enums
2. Test hook registration and execution flow
3. Test that hooks can deny tool execution
4. Test that multiple hooks execute in order

## Acceptance Criteria

- [ ] Hook module structure is created
- [ ] All core types compile and have proper derives
- [ ] Hook trait is async and thread-safe
- [ ] ToolRegistry can register and execute hooks
- [ ] Pre and post execution hooks are called
- [ ] Hooks can deny tool execution
- [ ] All tests pass

## Notes for Implementation

- Use `#[async_trait]` for the Hook trait
- Ensure all types used in Action enum have required derives (PartialEq, Eq, Serialize, Deserialize)
- The session_id should be generated when ToolRegistry is created (use uuid crate)
- Hook errors should not crash the application - log and continue
- This task focuses on the foundation - permission prompts will be implemented in a later task
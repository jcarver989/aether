# Task 05: Hook Manager and Registry

## Overview
Implement a HookManager that handles hook registration, execution ordering, and aggregation of hook results. Also complete the integration with ToolRegistry to support the full permission flow.

## Dependencies
- Task 02: Core Hook Infrastructure must be completed
- Task 03: Permission Hook Implementation must be completed

## Deliverables

### 1. Hook Manager Implementation (`src/hooks/manager.rs`)

Create a hook manager that orchestrates multiple hooks:

```rust
use super::{Hook, HookContext, HookResult, HookAction, HookResultContext};
use color_eyre::Result;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn, error};

pub struct HookManager {
    hooks: Vec<Arc<dyn Hook>>,
}

#[derive(Debug)]
pub struct AggregatedHookResult {
    pub action: HookAction,
    pub contexts: Vec<Option<HookResultContext>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
        }
    }
    
    pub fn register(&mut self, hook: Arc<dyn Hook>) {
        self.hooks.push(hook);
        debug!("Registered hook, total hooks: {}", self.hooks.len());
    }
    
    pub async fn run_pre_hooks(&self, context: &HookContext) -> Result<AggregatedHookResult> {
        debug!("Running pre-execution hooks for tool: {}", context.tool_name);
        
        let mut contexts = Vec::new();
        let mut modified_context = context.clone();
        
        for (idx, hook) in self.hooks.iter().enumerate() {
            match hook.pre_execute(&modified_context).await {
                Ok(result) => {
                    debug!("Hook {} returned action: {:?}", idx, result.action);
                    
                    match &result.action {
                        HookAction::Deny(reason) => {
                            warn!("Hook {} denied execution: {}", idx, reason);
                            return Ok(AggregatedHookResult {
                                action: result.action,
                                contexts: vec![result.context],
                            });
                        }
                        HookAction::RequireApproval => {
                            debug!("Hook {} requires approval", idx);
                            // Collect contexts from all hooks that ran so far
                            contexts.push(result.context);
                            // Continue running other hooks to collect more context
                        }
                        HookAction::ModifyArgs(new_args) => {
                            debug!("Hook {} modified arguments", idx);
                            modified_context.args = new_args.clone();
                            contexts.push(result.context);
                        }
                        HookAction::Allow => {
                            contexts.push(result.context);
                        }
                    }
                }
                Err(e) => {
                    error!("Hook {} error during pre_execute: {}", idx, e);
                    // Log error but continue with other hooks
                    // This ensures one faulty hook doesn't break everything
                }
            }
        }
        
        // If any hook required approval, return that
        let requires_approval = self.hooks.iter().enumerate().any(|(idx, hook)| {
            matches!(
                hook.pre_execute(&modified_context).await,
                Ok(HookResult { action: HookAction::RequireApproval, .. })
            )
        });
        
        if requires_approval {
            Ok(AggregatedHookResult {
                action: HookAction::RequireApproval,
                contexts,
            })
        } else {
            Ok(AggregatedHookResult {
                action: HookAction::Allow,
                contexts,
            })
        }
    }
    
    pub async fn run_post_hooks(&self, context: &HookContext, result: &Value) -> Result<()> {
        debug!("Running post-execution hooks for tool: {}", context.tool_name);
        
        for (idx, hook) in self.hooks.iter().enumerate() {
            if let Err(e) = hook.post_execute(context, result).await {
                error!("Hook {} error during post_execute: {}", idx, e);
                // Continue with other hooks even if one fails
            }
        }
        
        Ok(())
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}
```

### 2. Hook Rules and Patterns (`src/hooks/rules.rs`)

Add support for configurable hook rules:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRule {
    pub hook_type: String,
    pub pattern: HookPattern,
    pub enabled: bool,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum HookPattern {
    ToolName(String),
    ToolNamePattern(String),
    ServerName(String),
    All,
}

impl HookPattern {
    pub fn matches(&self, tool_name: &str, server_name: &str) -> bool {
        match self {
            HookPattern::ToolName(name) => tool_name == name,
            HookPattern::ToolNamePattern(pattern) => {
                // Simple glob matching
                if pattern.contains('*') {
                    let regex_pattern = pattern.replace("*", ".*");
                    regex::Regex::new(&format!("^{}$", regex_pattern))
                        .map(|re| re.is_match(tool_name))
                        .unwrap_or(false)
                } else {
                    tool_name == pattern
                }
            }
            HookPattern::ServerName(name) => server_name == name,
            HookPattern::All => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pattern_matching() {
        assert!(HookPattern::ToolName("write_file".to_string())
            .matches("write_file", "server1"));
        
        assert!(!HookPattern::ToolName("write_file".to_string())
            .matches("read_file", "server1"));
        
        assert!(HookPattern::ToolNamePattern("write_*".to_string())
            .matches("write_file", "server1"));
        
        assert!(HookPattern::ToolNamePattern("write_*".to_string())
            .matches("write_config", "server1"));
        
        assert!(HookPattern::ServerName("server1".to_string())
            .matches("any_tool", "server1"));
        
        assert!(HookPattern::All.matches("any_tool", "any_server"));
    }
}
```

### 3. Update ToolRegistry Integration (`src/mcp/registry.rs`)

Complete the ToolRegistry integration with proper permission handling:

```rust
use crate::hooks::{HookManager, HookAction, HookResultContext};
use crate::action::{Action, PermissionResponse};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

pub struct ToolRegistry {
    // ... existing fields ...
    hook_manager: HookManager,
    action_sender: Option<Sender<Action>>,
    session_id: String,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            // ... existing initialization ...
            hook_manager: HookManager::new(),
            action_sender: None,
            session_id: Uuid::new_v4().to_string(),
        }
    }
    
    pub fn register_hook(&mut self, hook: Arc<dyn Hook>) {
        self.hook_manager.register(hook);
    }
    
    pub fn set_action_sender(&mut self, sender: Sender<Action>) {
        self.action_sender = Some(sender);
    }
    
    pub async fn invoke_tool(&self, tool_name: &str, args: Value) -> Result<Value> {
        // Validate tool exists
        if !self.tools.contains_key(tool_name) {
            return Err(color_eyre::Report::msg(format!("Tool not found in registry: {}", tool_name)));
        }
        
        let server_name = self.get_server_for_tool(tool_name)
            .ok_or_else(|| color_eyre::Report::msg(format!("Server not found for tool: {}", tool_name)))?;
        
        // Create hook context
        let context = HookContext::new(
            tool_name.to_string(),
            server_name.to_string(),
            args.clone(),
            self.session_id.clone(),
        );
        
        // Run pre-execution hooks
        let hook_result = self.hook_manager.run_pre_hooks(&context).await?;
        
        // Handle hook results
        let final_args = match hook_result.action {
            HookAction::Allow => args,
            
            HookAction::Deny(reason) => {
                return Err(color_eyre::Report::msg(format!(
                    "Tool execution denied by hook: {}", reason
                )));
            }
            
            HookAction::RequireApproval => {
                // Send permission request to UI
                let action_sender = self.action_sender
                    .as_ref()
                    .ok_or_else(|| color_eyre::Report::msg("No action sender configured"))?;
                
                let (tx, rx) = tokio::sync::oneshot::channel::<PermissionResponse>();
                
                // Aggregate contexts for display
                let primary_context = hook_result.contexts.into_iter()
                    .flatten()
                    .next(); // Take first non-None context
                
                action_sender.send(Action::PromptPermission {
                    tool_name: tool_name.to_string(),
                    message: format!("Allow {} to execute?", tool_name),
                    context: primary_context,
                    callback: tx,
                }).await.map_err(|_| color_eyre::Report::msg("Failed to send permission request"))?;
                
                // Wait for user response
                match rx.await {
                    Ok(PermissionResponse::Approved) => {
                        tracing::info!("User approved execution of {}", tool_name);
                        args
                    }
                    Ok(PermissionResponse::Denied) => {
                        return Err(color_eyre::Report::msg(format!(
                            "User denied permission to execute {}", tool_name
                        )));
                    }
                    Ok(PermissionResponse::DeniedWithFeedback(feedback)) => {
                        return Err(color_eyre::Report::msg(format!(
                            "User denied permission: {}", feedback
                        )));
                    }
                    Err(_) => {
                        return Err(color_eyre::Report::msg("Permission request cancelled"));
                    }
                }
            }
            
            HookAction::ModifyArgs(new_args) => new_args,
        };
        
        // Execute the tool with final arguments
        let result = self.mcp_client
            .as_ref()
            .ok_or_else(|| color_eyre::Report::msg("No MCP client available"))?
            .execute_tool(server_name, tool_name, final_args)
            .await?;
        
        // Run post-execution hooks
        self.hook_manager.run_post_hooks(&context, &result).await?;
        
        Ok(result)
    }
}
```

### 4. Update Module Exports

Update `src/hooks/mod.rs`:
```rust
mod types;
mod context;
mod permission;
mod diff;
mod manager;
mod rules;

pub use types::*;
pub use context::*;
pub use permission::PermissionHook;
pub use diff::{generate_unified_diff, generate_diff_summary, DiffSummary};
pub use manager::{HookManager, AggregatedHookResult};
pub use rules::{HookRule, HookPattern};
```

## Testing Requirements

### Unit Tests for HookManager

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    
    struct TestHook {
        action: HookAction,
    }
    
    #[async_trait]
    impl Hook for TestHook {
        async fn pre_execute(&self, _context: &HookContext) -> Result<HookResult> {
            Ok(HookResult {
                action: self.action.clone(),
                context: None,
            })
        }
        
        async fn post_execute(&self, _context: &HookContext, _result: &Value) -> Result<()> {
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_hook_manager_allow() {
        let mut manager = HookManager::new();
        manager.register(Arc::new(TestHook { action: HookAction::Allow }));
        
        let context = HookContext::new(
            "test_tool".to_string(),
            "test_server".to_string(),
            serde_json::json!({}),
            "test_session".to_string(),
        );
        
        let result = manager.run_pre_hooks(&context).await.unwrap();
        assert!(matches!(result.action, HookAction::Allow));
    }
    
    #[tokio::test]
    async fn test_hook_manager_deny_stops_execution() {
        let mut manager = HookManager::new();
        manager.register(Arc::new(TestHook { action: HookAction::Allow }));
        manager.register(Arc::new(TestHook { 
            action: HookAction::Deny("test reason".to_string()) 
        }));
        manager.register(Arc::new(TestHook { action: HookAction::Allow }));
        
        let context = HookContext::new(
            "test_tool".to_string(),
            "test_server".to_string(),
            serde_json::json!({}),
            "test_session".to_string(),
        );
        
        let result = manager.run_pre_hooks(&context).await.unwrap();
        assert!(matches!(result.action, HookAction::Deny(_)));
    }
}
```

## Acceptance Criteria

- [ ] HookManager can register multiple hooks
- [ ] Hooks execute in registration order
- [ ] Deny action stops execution immediately
- [ ] RequireApproval collects contexts from all hooks
- [ ] Hook errors are logged but don't stop other hooks
- [ ] ToolRegistry properly integrates with HookManager
- [ ] Permission flow works end-to-end with action sender
- [ ] Modified arguments are passed to tool execution
- [ ] Post-execution hooks run after successful execution
- [ ] All tests pass

## Notes for Implementation

- Use `Arc<dyn Hook>` for thread-safe hook sharing
- Consider adding hook priorities in the future
- Log all hook decisions for debugging
- Ensure proper error handling throughout
- The session_id should be consistent for related operations
- Consider adding metrics for hook execution times
- Make sure the permission channel properly handles cancellation
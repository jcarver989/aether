# Task 03: Permission Hook Implementation

## Overview
Implement the PermissionHook that analyzes tool calls and determines which ones require user approval, with support for generating rich context like file diffs.

## Dependencies
- Task 02: Core Hook Infrastructure must be completed

## Deliverables

### 1. Permission Hook Implementation (`src/hooks/permission.rs`)

Create a permission hook that analyzes tool arguments and generates appropriate context:

```rust
use super::{Hook, HookContext, HookResult, HookAction, HookResultContext, FileOperation};
use async_trait::async_trait;
use color_eyre::Result;
use serde_json::Value;
use std::path::Path;

pub struct PermissionHook {
    sensitive_patterns: Vec<String>,
}

impl PermissionHook {
    pub fn new() -> Self {
        Self {
            sensitive_patterns: vec![
                "write_file".to_string(),
                "delete_file".to_string(),
                "execute_command".to_string(),
                "make_http_request".to_string(),
            ],
        }
    }
    
    pub fn with_patterns(patterns: Vec<String>) -> Self {
        Self {
            sensitive_patterns: patterns,
        }
    }
    
    fn is_sensitive_tool(&self, tool_name: &str) -> bool {
        self.sensitive_patterns.iter().any(|pattern| {
            if pattern.contains('*') {
                // Simple glob matching
                let regex_pattern = pattern.replace("*", ".*");
                regex::Regex::new(&format!("^{}$", regex_pattern))
                    .map(|re| re.is_match(tool_name))
                    .unwrap_or(false)
            } else {
                tool_name == pattern
            }
        })
    }
    
    async fn analyze_file_operation(&self, tool_name: &str, args: &Value) -> Result<Option<HookResultContext>> {
        match tool_name {
            "write_file" | "edit_file" => {
                let path = args.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| color_eyre::eyre::eyre!("Missing path argument"))?;
                
                let new_content = args.get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| color_eyre::eyre::eyre!("Missing content argument"))?;
                
                // Check if file exists and generate diff
                let (operation, diff) = if Path::new(path).exists() {
                    match tokio::fs::read_to_string(path).await {
                        Ok(current_content) => {
                            let diff = self.generate_diff(&current_content, new_content, path);
                            (FileOperation::Modify, diff)
                        }
                        Err(_) => {
                            // File exists but can't read - show as new file
                            (FileOperation::Create, format!("New file: {}\n{}", path, new_content))
                        }
                    }
                } else {
                    (FileOperation::Create, format!("New file: {}\n{}", path, new_content))
                };
                
                Ok(Some(HookResultContext::FileModification {
                    path: path.to_string(),
                    diff,
                    operation,
                }))
            }
            
            "delete_file" => {
                let path = args.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| color_eyre::eyre::eyre!("Missing path argument"))?;
                
                // Try to read file content for preview
                let diff = match tokio::fs::read_to_string(path).await {
                    Ok(content) => {
                        let preview = content.lines()
                            .take(20)
                            .map(|line| format!("- {}", line))
                            .collect::<Vec<_>>()
                            .join("\n");
                        
                        if content.lines().count() > 20 {
                            format!("{}\n... ({} more lines)", preview, content.lines().count() - 20)
                        } else {
                            preview
                        }
                    }
                    Err(_) => "Unable to preview file content".to_string(),
                };
                
                Ok(Some(HookResultContext::FileModification {
                    path: path.to_string(),
                    diff,
                    operation: FileOperation::Delete,
                }))
            }
            
            "execute_command" => {
                let command = args.get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| color_eyre::eyre::eyre!("Missing command argument"))?;
                
                let command_args = args.get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect())
                    .unwrap_or_default();
                
                let working_dir = args.get("working_dir")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                
                Ok(Some(HookResultContext::CommandExecution {
                    command: command.to_string(),
                    args: command_args,
                    working_dir,
                }))
            }
            
            _ => Ok(None),
        }
    }
    
    fn generate_diff(&self, old: &str, new: &str, filename: &str) -> String {
        // Note: This is a placeholder - actual implementation will use the 'similar' crate
        // This will be properly implemented in Task 04
        format!(
            "--- a/{}\n+++ b/{}\n@@ -1,{} +1,{} @@\n{}",
            filename,
            filename,
            old.lines().count(),
            new.lines().count(),
            "Diff generation will be implemented in Task 04"
        )
    }
}

#[async_trait]
impl Hook for PermissionHook {
    async fn pre_execute(&self, context: &HookContext) -> Result<HookResult> {
        // Check if this tool requires permission
        if !self.is_sensitive_tool(&context.tool_name) {
            return Ok(HookResult {
                action: HookAction::Allow,
                context: None,
            });
        }
        
        // Analyze the operation to generate context
        let result_context = self.analyze_file_operation(&context.tool_name, &context.args).await?;
        
        // Require approval for sensitive tools
        Ok(HookResult {
            action: HookAction::RequireApproval,
            context: result_context,
        })
    }
    
    async fn post_execute(&self, _context: &HookContext, _result: &Value) -> Result<()> {
        // Permission hook doesn't need post-execution logic
        Ok(())
    }
}
```

### 2. Add Permission Hook to Module (`src/hooks/mod.rs`)

```rust
mod types;
mod context;
mod permission;

pub use types::*;
pub use context::*;
pub use permission::PermissionHook;
```

### 3. Default Permission Patterns

Create a list of default sensitive tool patterns that should require permission:

```rust
impl Default for PermissionHook {
    fn default() -> Self {
        Self::with_patterns(vec![
            // File operations
            "write_file".to_string(),
            "create_file".to_string(),
            "delete_file".to_string(),
            "move_file".to_string(),
            "rename_file".to_string(),
            "append_to_file".to_string(),
            "edit_file".to_string(),
            
            // Directory operations
            "create_directory".to_string(),
            "delete_directory".to_string(),
            "move_directory".to_string(),
            
            // System operations
            "execute_command".to_string(),
            "run_script".to_string(),
            "kill_process".to_string(),
            
            // Network operations
            "make_http_request".to_string(),
            "download_file".to_string(),
            "upload_file".to_string(),
            
            // Pattern-based
            "*_destructive".to_string(),
            "sudo_*".to_string(),
            "*_system_*".to_string(),
        ])
    }
}
```

### 4. Integration Example

Show how to use the permission hook in the main application:

```rust
// In app initialization
let mut tool_registry = ToolRegistry::new();
let permission_hook = Box::new(PermissionHook::default());
tool_registry.register_hook(permission_hook);
```

## Testing Requirements

### Unit Tests (`src/hooks/permission.rs` - test module)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sensitive_tool_detection() {
        let hook = PermissionHook::default();
        
        assert!(hook.is_sensitive_tool("write_file"));
        assert!(hook.is_sensitive_tool("delete_file"));
        assert!(!hook.is_sensitive_tool("read_file"));
        assert!(!hook.is_sensitive_tool("list_files"));
    }
    
    #[test]
    fn test_glob_patterns() {
        let hook = PermissionHook::with_patterns(vec![
            "write_*".to_string(),
            "*_destructive".to_string(),
        ]);
        
        assert!(hook.is_sensitive_tool("write_file"));
        assert!(hook.is_sensitive_tool("write_config"));
        assert!(hook.is_sensitive_tool("delete_destructive"));
        assert!(!hook.is_sensitive_tool("read_file"));
    }
    
    #[tokio::test]
    async fn test_file_operation_analysis() {
        // Test analyze_file_operation for different tool types
    }
}
```

## Acceptance Criteria

- [ ] PermissionHook implements the Hook trait
- [ ] Sensitive tool detection works with exact matches and glob patterns
- [ ] File operations generate appropriate context (even if diff is placeholder)
- [ ] Command execution operations capture command details
- [ ] Default patterns cover common sensitive operations
- [ ] All tests pass
- [ ] Hook returns RequireApproval for sensitive tools
- [ ] Hook returns Allow for non-sensitive tools

## Notes for Implementation

- The `generate_diff` method is a placeholder that will be properly implemented in Task 04
- Use the `regex` crate for glob pattern matching
- File operations should handle both existing and non-existing files gracefully
- Consider file size limits when reading files for preview (e.g., skip files > 1MB)
- The hook should be defensive - errors in analysis shouldn't prevent tool execution
- Add appropriate logging for debugging hook decisions
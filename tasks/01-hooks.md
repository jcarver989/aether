# Task 01: Tool Execution Hooks System

## Overview

Implement a hooks system for MCP tool execution that allows intercepting tool calls before and after execution. This enables user permission prompts for sensitive operations (e.g., filesystem modification tools), logging, validation, and other custom behaviors.

## Current Architecture Analysis

### Tool Execution Flow
1. `ToolRegistry::invoke_tool(tool_name, args)` - Entry point in `src/mcp/registry.rs:80`
2. Tool validation and server lookup
3. `McpClient::execute_tool(server_name, tool_name, args)` - Execution in `src/mcp/client.rs:121`
4. Result returned to caller

### Integration Point
The ideal location for hooks is in `ToolRegistry::invoke_tool()` as it's the high-level entry point before delegation to the MCP client.

## Proposed Design

### Hook Types

1. **Permission Hooks** - Prompt user for approval of sensitive operations
2. **Logging Hooks** - Log tool executions for audit/debugging
3. **Validation Hooks** - Validate arguments before execution
4. **Rate Limiting Hooks** - Prevent excessive tool usage
5. **Notification Hooks** - User notifications about tool execution

### Hook Interface

```rust
#[async_trait::async_trait]
pub trait Hook: Send + Sync {
    /// Called before tool execution
    async fn pre_execute(&self, context: &HookContext) -> Result<HookResult>;
    
    /// Called after tool execution
    async fn post_execute(&self, context: &HookContext, result: &Value) -> Result<()>;
}

pub struct HookResult {
    pub action: HookAction,
    pub context: Option<HookResultContext>,
}

pub enum HookAction {
    Allow,                      // Proceed with execution
    Deny(String),              // Block execution with reason  
    RequireApproval,           // Require user approval
    ModifyArgs(Value),         // Modify arguments before execution
}

pub enum HookResultContext {
    /// File modification context with diff
    FileModification {
        path: String,
        diff: String,              // Unified diff format
        operation: FileOperation,
    },
    /// Multiple file modifications
    BatchFileModification {
        operations: Vec<FileModificationInfo>,
    },
    /// Command execution context
    CommandExecution {
        command: String,
        args: Vec<String>,
        working_dir: Option<String>,
    },
    /// Network request context
    NetworkRequest {
        url: String,
        method: String,
        headers: Option<HashMap<String, String>>,
    },
    /// Custom context for other tools
    Custom(Value),
}

pub enum FileOperation {
    Create,
    Modify,
    Delete,
    Rename { from: String, to: String },
}

pub struct FileModificationInfo {
    pub path: String,
    pub operation: FileOperation,
    pub size_bytes: Option<u64>,
    pub preview: Option<String>,  // First N lines for preview
}

pub struct HookContext {
    pub tool_name: String,
    pub server_name: String,
    pub args: Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub session_id: String,        // For correlating related operations
}
```

### Hook Manager

```rust
pub struct HookManager {
    hooks: Vec<Box<dyn Hook>>,
    rules: Vec<HookRule>,
}

pub struct HookRule {
    pub hook_id: String,
    pub pattern: HookPattern,
    pub enabled: bool,
}

pub enum HookPattern {
    ToolName(String),           // Exact tool name
    ToolNamePattern(String),    // Glob pattern like "write_*"
    ServerName(String),         // MCP server name
    All,                        // Apply to all tools
}
```

### Configuration

Hooks should be configurable via `mcp.json` or a separate hooks config:

```json
{
  "hooks": {
    "permission_hooks": [
      {
        "pattern": "write_*",
        "description": "File write operations require permission",
        "prompt": "Allow {tool_name} to write files?"
      },
      {
        "pattern": "delete_*", 
        "description": "File deletion requires permission",
        "prompt": "Allow {tool_name} to delete files?"
      }
    ],
    "logging": {
      "enabled": true,
      "log_all_tools": true,
      "log_file": "tool_executions.log"
    }
  }
}
```

## Implementation Plan

### Phase 1: Core Hook Infrastructure
1. Create `src/hooks/` module with core traits and types
2. Implement `HookManager` and basic hook registry
3. Add hook integration points to `ToolRegistry::invoke_tool()`
4. Create configuration loading for hooks

### Phase 2: Built-in Hook Implementations  
1. **PermissionHook** - Interactive user prompts for sensitive tools
   - Analyzes tool arguments to generate contextual information
   - For file operations, generates diffs by reading current content
   - Returns `RequireApproval` with rich context
2. **LoggingHook** - File-based logging of all tool executions
3. **ValidationHook** - Basic argument validation
4. **DiffGeneratorHook** - Specialized hook for generating file diffs

### Phase 3: UI Integration
1. Add permission prompt UI component using existing action system
2. Integrate with main app event loop for user interaction
3. Add hook status display in UI

### Phase 4: Configuration & Polish
1. Add hook configuration to existing config system
2. Add hook enable/disable controls
3. Add documentation and examples

## File Structure

```
src/
├── hooks/
│   ├── mod.rs              # Public interface and re-exports
│   ├── manager.rs          # HookManager implementation
│   ├── types.rs            # Hook traits and types
│   ├── permission.rs       # PermissionHook implementation
│   ├── logging.rs          # LoggingHook implementation
│   └── validation.rs       # ValidationHook implementation
├── mcp/
│   ├── registry.rs         # Modified to integrate hooks
│   └── ...
└── components/
    └── permission_prompt.rs # UI for permission requests
```

## Integration with Action System

Permission hooks should emit actions for user interaction:

```rust
pub enum Action {
    // Existing actions...
    
    // Hook-related actions
    PromptPermission {
        tool_name: String,
        message: String,
        context: Option<HookResultContext>,
        callback: tokio::sync::oneshot::Sender<bool>,
    },
    DismissPermissionPrompt,
}
```

## Example: File Write Hook Flow

Here's how a file write operation would work with hooks:

```rust
// 1. LLM calls write_file tool
let args = json!({
    "path": "/src/main.rs",
    "content": "fn main() {\n    println!(\"Hello, hooks!\");\n}"
});

// 2. PermissionHook analyzes the request
impl Hook for PermissionHook {
    async fn pre_execute(&self, context: &HookContext) -> Result<HookResult> {
        if context.tool_name == "write_file" {
            let path = context.args["path"].as_str().unwrap();
            let new_content = context.args["content"].as_str().unwrap();
            
            // Read current content if file exists
            let current_content = tokio::fs::read_to_string(path).await.ok();
            
            // Generate diff
            let diff = if let Some(current) = current_content {
                generate_unified_diff(&current, new_content, path)
            } else {
                format!("New file: {}\n{}", path, new_content)
            };
            
            Ok(HookResult {
                action: HookAction::RequireApproval,
                context: Some(HookResultContext::FileModification {
                    path: path.to_string(),
                    diff,
                    operation: if current_content.is_some() { 
                        FileOperation::Modify 
                    } else { 
                        FileOperation::Create 
                    },
                }),
            })
        } else {
            Ok(HookResult { action: HookAction::Allow, context: None })
        }
    }
}

// 3. UI displays the diff to user
// The permission prompt component would show:
// - Tool name and description
// - File path being modified
// - Unified diff showing exact changes
// - Accept/Reject buttons

// 4. User approves/rejects
// If approved, tool execution continues
// If rejected, returns error to LLM
```

## Complete Permission Flow

Here's the complete flow when a hook requires user approval:

```rust
// In ToolRegistry::invoke_tool()
pub async fn invoke_tool(&self, tool_name: &str, args: Value) -> Result<Value> {
    // 1. Run pre-execution hooks
    let hook_result = self.hook_manager.run_pre_hooks(&HookContext {
        tool_name: tool_name.to_string(),
        server_name: self.get_server_for_tool(tool_name)?.to_string(),
        args: args.clone(),
        timestamp: chrono::Utc::now(),
        session_id: self.session_id.clone(),
    }).await?;
    
    // 2. Handle hook results
    match hook_result.action {
        HookAction::Allow => {
            // Continue with tool execution
        }
        HookAction::Deny(reason) => {
            // Return error immediately
            return Err(color_eyre::Report::msg(format!(
                "Tool execution denied by hook: {}", reason
            )));
        }
        HookAction::RequireApproval => {
            // 3. Create permission request
            let (tx, rx) = tokio::sync::oneshot::channel::<PermissionResponse>();
            
            // 4. Send action to UI
            self.action_sender.send(Action::PromptPermission {
                tool_name: tool_name.to_string(),
                message: format!("Allow {} to execute?", tool_name),
                context: hook_result.context,
                callback: tx,
            })?;
            
            // 5. Wait for user response
            match rx.await? {
                PermissionResponse::Approved => {
                    // User approved - continue with execution
                    tracing::info!("User approved execution of {}", tool_name);
                }
                PermissionResponse::Denied => {
                    // User denied - return error to LLM
                    return Err(color_eyre::Report::msg(format!(
                        "User denied permission to execute {}", tool_name
                    )));
                }
                PermissionResponse::DeniedWithFeedback(feedback) => {
                    // User denied with additional context
                    return Err(color_eyre::Report::msg(format!(
                        "User denied permission: {}", feedback
                    )));
                }
            }
        }
        HookAction::ModifyArgs(new_args) => {
            // Use modified args for execution
            args = new_args;
        }
    }
    
    // 6. Execute the tool (only reached if allowed/approved)
    let result = self.mcp_client.execute_tool(
        self.get_server_for_tool(tool_name)?,
        tool_name,
        args
    ).await?;
    
    // 7. Run post-execution hooks
    self.hook_manager.run_post_hooks(&HookContext { /* ... */ }, &result).await?;
    
    Ok(result)
}

pub enum PermissionResponse {
    Approved,
    Denied,
    DeniedWithFeedback(String),
}
```

## Diff Generation and Rendering

### Diff Generation using `similar` crate

```toml
# Cargo.toml
[dependencies]
similar = { version = "2.5", features = ["text", "bytes"] }
```

```rust
use similar::{TextDiff, ChangeTag};

pub fn generate_unified_diff(old: &str, new: &str, filename: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    
    let mut output = String::new();
    output.push_str(&format!("--- a/{}\n", filename));
    output.push_str(&format!("+++ b/{}\n", filename));
    
    for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
        if idx > 0 {
            output.push_str("...\n");
        }
        for op in group {
            for change in diff.iter_inline_changes(op) {
                let (sign, style) = match change.tag() {
                    ChangeTag::Delete => ("-", "delete"),
                    ChangeTag::Insert => ("+", "insert"),
                    ChangeTag::Equal => (" ", "equal"),
                };
                output.push_str(&format!("{} {}", sign, change));
            }
        }
    }
    output
}
```

## UI Component for Permission Prompts

```rust
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub struct PermissionPrompt {
    tool_name: String,
    message: String,
    context: Option<HookResultContext>,
    callback: Option<tokio::sync::oneshot::Sender<PermissionResponse>>,
    scroll_offset: u16,
    diff_lines: Vec<DiffLine>,  // Pre-processed diff lines
}

#[derive(Clone)]
struct DiffLine {
    content: String,
    line_type: DiffLineType,
}

#[derive(Clone, Copy)]
enum DiffLineType {
    Added,
    Removed,
    Context,
    Header,
}

impl PermissionPrompt {
    pub fn new(
        tool_name: String,
        message: String,
        context: Option<HookResultContext>,
        callback: tokio::sync::oneshot::Sender<PermissionResponse>,
    ) -> Self {
        let mut prompt = Self {
            tool_name,
            message,
            context,
            callback: Some(callback),
            scroll_offset: 0,
            diff_lines: Vec::new(),
        };
        
        // Pre-process diff for rendering
        if let Some(HookResultContext::FileModification { diff, .. }) = &prompt.context {
            prompt.diff_lines = Self::parse_diff(diff);
        }
        
        prompt
    }
    
    fn parse_diff(diff: &str) -> Vec<DiffLine> {
        diff.lines().map(|line| {
            let line_type = match line.chars().next() {
                Some('+') if !line.starts_with("+++") => DiffLineType::Added,
                Some('-') if !line.starts_with("---") => DiffLineType::Removed,
                Some('@') => DiffLineType::Header,
                _ if line.starts_with("+++") || line.starts_with("---") => DiffLineType::Header,
                _ => DiffLineType::Context,
            };
            DiffLine {
                content: line.to_string(),
                line_type,
            }
        }).collect()
    }
    
    fn render_diff(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();
        
        // Add file path header
        if let Some(HookResultContext::FileModification { path, operation, .. }) = &self.context {
            let op_text = match operation {
                FileOperation::Create => "CREATE",
                FileOperation::Modify => "MODIFY",
                FileOperation::Delete => "DELETE",
                FileOperation::Rename { from, to } => &format!("RENAME {} → {}", from, to),
            };
            
            lines.push(Line::from(vec![
                Span::styled(op_text, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(path, Style::default().fg(Color::Cyan)),
            ]));
            lines.push(Line::from(""));
        }
        
        // Render diff lines with syntax highlighting
        for diff_line in self.diff_lines.iter().skip(self.scroll_offset as usize) {
            let style = match diff_line.line_type {
                DiffLineType::Added => Style::default().fg(Color::Green),
                DiffLineType::Removed => Style::default().fg(Color::Red),
                DiffLineType::Header => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                DiffLineType::Context => Style::default().fg(Color::Gray),
            };
            
            lines.push(Line::from(Span::styled(&diff_line.content, style)));
        }
        
        let diff_widget = Paragraph::new(lines)
            .block(Block::default()
                .title("File Changes")
                .borders(Borders::ALL))
            .wrap(Wrap { trim: false });
            
        diff_widget.render(area, buf);
    }
    
    fn render_controls(&self, area: Rect, buf: &mut Buffer) {
        let controls = vec![
            Line::from(vec![
                Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" - Approve"),
            ]),
            Line::from(vec![
                Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" - Deny"),
            ]),
            Line::from(vec![
                Span::styled("f", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" - Deny with feedback"),
            ]),
            Line::from(vec![
                Span::styled("↑/↓", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Scroll diff"),
            ]),
        ];
        
        let controls_widget = Paragraph::new(controls)
            .block(Block::default()
                .title("Controls")
                .borders(Borders::ALL));
                
        controls_widget.render(area, buf);
    }
    
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                // Send approval
                if let Some(cb) = self.callback.take() {
                    let _ = cb.send(PermissionResponse::Approved);
                }
                Ok(Some(Action::DismissPermissionPrompt))
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                // Send denial
                if let Some(cb) = self.callback.take() {
                    let _ = cb.send(PermissionResponse::Denied);
                }
                Ok(Some(Action::DismissPermissionPrompt))
            }
            KeyCode::Char('f') => {
                // Deny with feedback
                Ok(Some(Action::OpenFeedbackInput))
            }
            KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                Ok(None)
            }
            KeyCode::Down => {
                if self.scroll_offset < self.diff_lines.len().saturating_sub(10) as u16 {
                    self.scroll_offset += 1;
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

impl Component for PermissionPrompt {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Layout: 
        // - Title and message (20%)
        // - Diff view (60%)
        // - Controls (20%)
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(area);
            
        // Render title and message
        let title_text = vec![
            Line::from(Span::styled(
                &self.tool_name,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(&self.message),
        ];
        
        Paragraph::new(title_text)
            .block(Block::default().borders(Borders::ALL))
            .render(chunks[0], buf);
            
        // Render diff
        self.render_diff(chunks[1], buf);
        
        // Render controls
        self.render_controls(chunks[2], buf);
    }
}
```

## Error Handling and LLM Communication

When a tool execution is denied, the error should be informative for the LLM:

```rust
// Example error messages returned to LLM:

// User denied
"Tool execution denied: User rejected write_file operation on /etc/passwd"

// Hook denied automatically
"Tool execution denied: Attempting to modify system file /etc/passwd is not allowed"

// With user feedback
"Tool execution denied: User comment: 'This would break the production database configuration'"
```

The LLM can then:
1. Understand why the operation was denied
2. Suggest alternative approaches
3. Ask for clarification from the user
4. Modify its approach based on the feedback

## Hook Chaining and Composition

Multiple hooks can be registered for the same tool, executing in order:

```rust
impl HookManager {
    pub async fn run_pre_hooks(&self, context: &HookContext) -> Result<AggregatedHookResult> {
        let mut results = Vec::new();
        
        for hook in &self.hooks {
            match hook.pre_execute(context).await {
                Ok(result) => {
                    // Stop on first Deny or RequireApproval
                    match result.action {
                        HookAction::Deny(_) | HookAction::RequireApproval => {
                            return Ok(AggregatedHookResult {
                                action: result.action,
                                contexts: vec![result.context],
                            });
                        }
                        HookAction::ModifyArgs(args) => {
                            // Update context for next hook
                            context = HookContext { args, ..context };
                        }
                        _ => {}
                    }
                    results.push(result);
                }
                Err(e) => {
                    // Log error but continue with other hooks
                    tracing::error!("Hook error: {}", e);
                }
            }
        }
        
        // Aggregate all contexts for UI display
        Ok(AggregatedHookResult::from_results(results))
    }
}
```

## Performance Considerations

1. **Lazy Diff Generation** - Only read files and generate diffs when RequireApproval is needed
2. **Diff Size Limits** - Truncate large diffs with option to view full diff
3. **Caching** - Cache file contents during a session to avoid repeated reads
4. **Async Execution** - Run independent hooks in parallel when possible

## Security Considerations

1. **Default Deny** - Unknown/unconfigured sensitive tools should be denied by default
2. **User Control** - Users should be able to configure which tools require permissions
3. **Audit Trail** - All tool executions should be logged when hooks are enabled
4. **Hook Isolation** - Hook failures shouldn't prevent tool execution unless explicitly configured
5. **Path Validation** - Hooks should validate file paths to prevent directory traversal

## Testing Strategy

1. **Unit Tests** - Test individual hook implementations
2. **Integration Tests** - Test hook manager with mock tools
3. **UI Tests** - Test permission prompt component
4. **End-to-End Tests** - Test full flow with real MCP servers

## Future Enhancements

1. **Custom Hooks** - Plugin system for user-defined hooks
2. **Policy Engine** - Rule-based policy system for complex scenarios
3. **Remote Approval** - Integration with external approval systems
4. **Metrics** - Tool usage analytics and monitoring
5. **Batch Operations** - Handle multiple tool calls with single permission

## Success Criteria

- [ ] Sensitive tools (write_file, delete_file, etc.) prompt for user permission
- [ ] All tool executions are logged when logging is enabled
- [ ] Hooks are configurable via JSON configuration
- [ ] Hook system integrates cleanly with existing action-based architecture
- [ ] Performance impact is minimal (< 10ms overhead per tool call)
- [ ] Comprehensive test coverage (> 80%)
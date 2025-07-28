# Task 08: Hook Testing Infrastructure

## Overview
Create comprehensive testing infrastructure for the hooks system, including test utilities, fake tool implementations, and integration tests that use real hook implementations.

## Dependencies
- All previous hook tasks (02-07) should be completed

## Deliverables

### 1. Test Utilities Module (`src/hooks/test_utils.rs`)

Create utilities for testing hooks:

```rust
use super::{Hook, HookContext, HookResult, HookAction, HookResultContext};
use crate::mcp::{registry::ToolRegistry, client::McpClient};
use async_trait::async_trait;
use color_eyre::Result;
use rmcp::{
    model::{CallToolRequestParam, CallToolResult, Content, Tool as RmcpTool},
    transport::Transport,
    RoleServer, ServerInfo, Implementation,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

/// In-memory transport for testing MCP connections
#[derive(Clone)]
pub struct InMemoryTransport {
    sender: mpsc::UnboundedSender<Vec<u8>>,
    receiver: Arc<Mutex<mpsc::UnboundedReceiver<Vec<u8>>>>,
}

impl InMemoryTransport {
    pub fn pair() -> (Self, Self) {
        let (tx1, rx1) = mpsc::unbounded_channel();
        let (tx2, rx2) = mpsc::unbounded_channel();
        
        let transport1 = Self {
            sender: tx1,
            receiver: Arc::new(Mutex::new(rx2)),
        };
        
        let transport2 = Self {
            sender: tx2,
            receiver: Arc::new(Mutex::new(rx1)),
        };
        
        (transport1, transport2)
    }
}

#[async_trait]
impl Transport for InMemoryTransport {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    
    async fn send(&self, data: &[u8]) -> Result<(), Self::Error> {
        self.sender.send(data.to_vec())
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
    
    async fn receive(&self) -> Result<Vec<u8>, Self::Error> {
        let mut receiver = self.receiver.lock().unwrap();
        receiver.recv().await
            .ok_or_else(|| "Channel closed".into())
    }
}

/// In-memory file system for testing file operations
#[derive(Debug, Clone)]
pub struct InMemoryFileSystem {
    files: Arc<Mutex<HashMap<String, String>>>,
}

impl InMemoryFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub fn write_file(&self, path: &str, content: &str) {
        self.files.lock().unwrap().insert(path.to_string(), content.to_string());
    }
    
    pub fn read_file(&self, path: &str) -> Option<String> {
        self.files.lock().unwrap().get(path).cloned()
    }
    
    pub fn exists(&self, path: &str) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }
    
    pub fn remove_file(&self, path: &str) -> bool {
        self.files.lock().unwrap().remove(path).is_some()
    }
    
    pub fn list_files(&self) -> Vec<String> {
        self.files.lock().unwrap().keys().cloned().collect()
    }
}

/// In-memory command executor for testing
#[derive(Debug, Clone)]
pub struct InMemoryCommandExecutor {
    executions: Arc<Mutex<Vec<(String, Vec<String>)>>>,
}

impl InMemoryCommandExecutor {
    pub fn new() -> Self {
        Self {
            executions: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn execute(&self, command: &str, args: &[String]) -> String {
        self.executions.lock().unwrap().push((command.to_string(), args.to_vec()));
        format!("Executed: {} {}", command, args.join(" "))
    }
    
    pub fn get_executions(&self) -> Vec<(String, Vec<String>)> {
        self.executions.lock().unwrap().clone()
    }
}

/// Test MCP server using real rmcp SDK with in-memory tools
pub struct TestMcpServer {
    filesystem: InMemoryFileSystem,
    command_executor: InMemoryCommandExecutor,
}

impl TestMcpServer {
    pub fn new() -> Self {
        Self {
            filesystem: InMemoryFileSystem::new(),
            command_executor: InMemoryCommandExecutor::new(),
        }
    }
    
    pub fn filesystem(&self) -> &InMemoryFileSystem {
        &self.filesystem
    }
    
    pub fn command_executor(&self) -> &InMemoryCommandExecutor {
        &self.command_executor
    }
    
    /// Create a real MCP server instance using rmcp SDK
    pub async fn serve(self, transport: InMemoryTransport) -> Result<()> {
        let server_info = ServerInfo {
            name: "test-server".to_string(),
            version: "1.0.0".to_string(),
            protocol_version: rmcp::PROTOCOL_VERSION.to_string(),
            capabilities: rmcp::ServerCapabilities {
                tools: Some(rmcp::ToolsCapability { list_changed: Some(false) }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "test-mcp-server".to_string(),
                version: "1.0.0".to_string(),
            },
        };
        
        // Create the real MCP server with our tools
        let server = rmcp::serve_server(server_info, transport, |server| {
            let fs = self.filesystem.clone();
            let cmd_exec = self.command_executor.clone();
            
            // Register write_file tool
            server.tool("write_file", "Write content to a file", |params| {
                let fs = fs.clone();
                async move {
                    let path = params.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or("Missing path parameter")?;
                    let content = params.get("content")
                        .and_then(|v| v.as_str())
                        .ok_or("Missing content parameter")?;
                    
                    fs.write_file(path, content);
                    
                    Ok(CallToolResult {
                        content: vec![Content::Text {
                            text: format!("Successfully wrote {} bytes to {}", content.len(), path),
                        }],
                        is_error: Some(false),
                    })
                }
            })?;
            
            // Register read_file tool
            server.tool("read_file", "Read content from a file", |params| {
                let fs = fs.clone();
                async move {
                    let path = params.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or("Missing path parameter")?;
                    
                    match fs.read_file(path) {
                        Some(content) => Ok(CallToolResult {
                            content: vec![Content::Text { text: content }],
                            is_error: Some(false),
                        }),
                        None => Ok(CallToolResult {
                            content: vec![Content::Text {
                                text: format!("File not found: {}", path),
                            }],
                            is_error: Some(true),
                        }),
                    }
                }
            })?;
            
            // Register execute_command tool
            server.tool("execute_command", "Execute a system command", |params| {
                let cmd_exec = cmd_exec.clone();
                async move {
                    let command = params.get("command")
                        .and_then(|v| v.as_str())
                        .ok_or("Missing command parameter")?;
                    let args = params.get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect())
                        .unwrap_or_default();
                    
                    let output = cmd_exec.execute(command, &args);
                    
                    Ok(CallToolResult {
                        content: vec![Content::Text { text: output }],
                        is_error: Some(false),
                    })
                }
            })?;
            
            Ok(())
        }).await?;
        
        // Run the server
        server.run().await?;
        
        Ok(())
    }
}

/// Test harness that connects real hooks to real MCP server with in-memory tools
pub struct HookTestHarness {
    pub tool_registry: ToolRegistry,
    pub mcp_client: Arc<McpClient>,
    pub filesystem: InMemoryFileSystem,
    pub command_executor: InMemoryCommandExecutor,
    _server_handle: tokio::task::JoinHandle<Result<()>>,
}

impl HookTestHarness {
    pub async fn new() -> Result<Self> {
        let (client_transport, server_transport) = InMemoryTransport::pair();
        
        // Create test MCP server with in-memory tools
        let test_server = TestMcpServer::new();
        let filesystem = test_server.filesystem().clone();
        let command_executor = test_server.command_executor().clone();
        
        // Start the real MCP server in the background
        let server_handle = tokio::spawn(async move {
            test_server.serve(server_transport).await
        });
        
        // Create real MCP client connected to our test server
        let mut mcp_client = McpClient::new();
        
        // Connect using the in-memory transport
        // Note: This might require modifying McpClient to accept custom transports
        // For now, we'll assume a method like connect_with_transport exists
        mcp_client.connect_with_transport(
            "test_server".to_string(),
            client_transport,
        ).await?;
        let mcp_client = Arc::new(mcp_client);
        
        // Create tool registry and connect MCP client
        let mut tool_registry = ToolRegistry::new();
        tool_registry.set_mcp_client(mcp_client.clone());
        
        // Discover and register tools from the real MCP server
        let discovered_tools = mcp_client.discover_tools().await?;
        for (server_name, tool) in discovered_tools {
            tool_registry.register_tool(server_name, tool);
        }
        
        Ok(Self {
            tool_registry,
            mcp_client,
            filesystem,
            command_executor,
            _server_handle: server_handle,
        })
    }
    
    pub fn register_real_hook(mut self, hook: Arc<dyn Hook>) -> Self {
        self.tool_registry.register_hook(hook);
        self
    }
    
    pub async fn execute_tool_with_hooks(
        &mut self,
        tool_name: &str,
        args: Value,
    ) -> Result<Value> {
        // This invokes the full pipeline:
        // 1. Hook pre_execute (may require user approval, generates real diffs)
        // 2. Real MCP tool execution via real server with in-memory storage
        // 3. Hook post_execute
        self.tool_registry.invoke_tool(tool_name, args).await
    }
    
    /// Setup a file in the in-memory filesystem for testing
    pub fn setup_file(&self, path: &str, content: &str) {
        self.filesystem.write_file(path, content);
    }
    
    /// Get file content from in-memory filesystem
    pub fn get_file_content(&self, path: &str) -> Option<String> {
        self.filesystem.read_file(path)
    }
    
    /// Check if file exists in in-memory filesystem
    pub fn file_exists(&self, path: &str) -> bool {
        self.filesystem.exists(path)
    }
    
    /// Get all executed commands
    pub fn get_executed_commands(&self) -> Vec<(String, Vec<String>)> {
        self.command_executor.get_executions()
    }
    
    pub fn create_file_write_args(path: &str, content: &str) -> Value {
        serde_json::json!({
            "path": path,
            "content": content
        })
    }
    
    pub fn create_command_args(command: &str, args: Vec<&str>) -> Value {
        serde_json::json!({
            "command": command,
            "args": args
        })
    }
}


/// Test permission responder that automatically approves/denies
pub struct TestPermissionResponder {
    auto_response: crate::action::PermissionResponse,
}

impl TestPermissionResponder {
    pub fn auto_approve() -> Self {
        Self {
            auto_response: crate::action::PermissionResponse::Approved,
        }
    }
    
    pub fn auto_deny() -> Self {
        Self {
            auto_response: crate::action::PermissionResponse::Denied,
        }
    }
    
    pub fn auto_deny_with_feedback(feedback: impl Into<String>) -> Self {
        Self {
            auto_response: crate::action::PermissionResponse::DeniedWithFeedback(feedback.into()),
        }
    }
    
    pub fn setup_action_sender(&self, harness: &mut HookTestHarness) -> mpsc::Receiver<crate::action::Action> {
        let (tx, rx) = mpsc::channel(100);
        harness.tool_registry.set_action_sender(tx);
        
        // Spawn a task to auto-respond to permission requests
        let response = self.auto_response.clone();
        tokio::spawn(async move {
            while let Some(action) = rx.recv().await {
                if let crate::action::Action::PromptPermission { callback, .. } = action {
                    let _ = callback.send(response.clone());
                }
            }
        });
        
        rx
    }
}
```

### 2. Integration Tests (`tests/hook_integration.rs`)

Create comprehensive integration tests using real hooks and real MCP servers with in-memory storage:

```rust
use aether::hooks::{
    PermissionHook, HookAction, HookResultContext,
    PermissionHookConfig, ToolPattern, DefaultBehavior,
};
use aether::hooks::test_utils::{HookTestHarness, TestPermissionResponder};
use std::sync::Arc;

#[tokio::test]
async fn test_real_permission_hook_with_file_write() {
    // Create real permission hook with default config
    let permission_hook = Arc::new(PermissionHook::default());
    
    // Set up test harness with real MCP server and in-memory tools
    let mut harness = HookTestHarness::new().await.unwrap()
        .register_real_hook(permission_hook);
    
    // Setup initial file content in in-memory filesystem
    harness.setup_file("/test.txt", "original content");
    
    // Set up auto-approval for permission requests
    let responder = TestPermissionResponder::auto_approve();
    let _action_rx = responder.setup_action_sender(&mut harness);
    
    // Execute write_file tool - this goes through the full pipeline
    let args = HookTestHarness::create_file_write_args(
        "/test.txt",
        "new content from hook test"
    );
    
    let result = harness.execute_tool_with_hooks("write_file", args).await.unwrap();
    
    // Verify the tool actually executed and wrote to in-memory filesystem
    let file_content = harness.get_file_content("/test.txt").unwrap();
    assert_eq!(file_content, "new content from hook test");
    
    // Verify the result indicates success
    assert!(result.to_string().contains("Successfully wrote"));
}

#[tokio::test]
async fn test_permission_hook_generates_real_diff() {
    // Create permission hook that generates real diffs
    let permission_hook = Arc::new(PermissionHook::default());
    
    let mut harness = HookTestHarness::new().await.unwrap()
        .register_real_hook(permission_hook);
    
    // Setup initial file with Rust code
    harness.setup_file("/example.rs", "fn old() {\n    println!(\"old\");\n}");
    
    // Set up auto-denial to capture the hook result without executing
    let responder = TestPermissionResponder::auto_deny();
    let mut action_rx = responder.setup_action_sender(&mut harness);
    
    let args = HookTestHarness::create_file_write_args(
        "/example.rs",
        "fn new() {\n    println!(\"new and improved\");\n}"
    );
    
    // Try to execute - should fail due to denial but we can capture the diff
    let result = harness.execute_tool_with_hooks("write_file", args).await;
    assert!(result.is_err()); // Should be denied
    assert!(result.unwrap_err().to_string().contains("denied"));
    
    // Verify that permission prompt was sent with real diff
    if let Some(action) = action_rx.recv().await {
        if let crate::action::Action::PromptPermission { context, .. } = action {
            if let Some(HookResultContext::FileModification { diff, .. }) = context {
                assert!(diff.contains("fn old()"));
                assert!(diff.contains("fn new()"));
                assert!(diff.contains("--- a/"));
                assert!(diff.contains("+++ b/"));
                assert!(diff.contains("println!(\"old\")"));
                assert!(diff.contains("println!(\"new and improved\")"));
            } else {
                panic!("Expected FileModification context");
            }
        } else {
            panic!("Expected PromptPermission action");
        }
    }
    
    // Verify file was not modified due to denial
    let original_content = harness.get_file_content("/example.rs").unwrap();
    assert!(original_content.contains("fn old()"));
    assert!(!original_content.contains("fn new()"));
}

#[tokio::test]
async fn test_permission_hook_allows_safe_tools() {
    // Create permission hook that allows read operations
    let config = PermissionHookConfig {
        enabled: true,
        require_approval: vec![
            ToolPattern::Pattern { pattern: "write_*".to_string() },
        ],
        always_allow: vec![
            ToolPattern::Pattern { pattern: "read_*".to_string() },
        ],
        always_deny: vec![],
        custom_messages: Default::default(),
    };
    
    let permission_hook = Arc::new(PermissionHook::from_config(
        config,
        DefaultBehavior::RequireApproval,
    ));
    
    let mut harness = HookTestHarness::new().await.unwrap()
        .register_real_hook(permission_hook);
    
    // Setup file in in-memory filesystem
    harness.setup_file("/read_test.txt", "content to read");
    
    // No need for permission responder since read should be auto-allowed
    
    let args = serde_json::json!({
        "path": "/read_test.txt"
    });
    
    let result = harness.execute_tool_with_hooks("read_file", args).await.unwrap();
    
    // Verify the file was actually read from in-memory filesystem
    assert!(result.to_string().contains("content to read"));
}

#[tokio::test]
async fn test_permission_hook_denies_dangerous_tools() {
    // Create permission hook that denies command execution
    let config = PermissionHookConfig {
        enabled: true,
        require_approval: vec![],
        always_allow: vec![],
        always_deny: vec![
            ToolPattern::Pattern { pattern: "execute_*".to_string() },
        ],
        custom_messages: Default::default(),
    };
    
    let permission_hook = Arc::new(PermissionHook::from_config(
        config,
        DefaultBehavior::Allow,
    ));
    
    let mut harness = HookTestHarness::new().await.unwrap()
        .register_real_hook(permission_hook);
    
    let args = HookTestHarness::create_command_args("rm", vec!["-rf", "/"]);
    
    let result = harness.execute_tool_with_hooks("execute_command", args).await;
    
    // Should be denied by hook before reaching the in-memory executor
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("denied"));
    
    // Verify no commands were executed
    assert!(harness.get_executed_commands().is_empty());
}

#[tokio::test]
async fn test_permission_hook_with_user_denial() {
    let permission_hook = Arc::new(PermissionHook::default());
    
    let mut harness = HookTestHarness::new().await.unwrap()
        .register_real_hook(permission_hook);
    
    // Set up auto-denial with feedback
    let responder = TestPermissionResponder::auto_deny_with_feedback("This looks suspicious");
    let _action_rx = responder.setup_action_sender(&mut harness);
    
    let args = HookTestHarness::create_file_write_args(
        "/sensitive_file.txt",
        "malicious content"
    );
    
    let result = harness.execute_tool_with_hooks("write_file", args).await;
    
    // Should be denied with user feedback
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("This looks suspicious"));
    
    // Verify file was not created in in-memory filesystem
    assert!(!harness.file_exists("/sensitive_file.txt"));
}

#[tokio::test]
async fn test_command_execution_tracking() {
    let permission_hook = Arc::new(PermissionHook::default());
    
    let mut harness = HookTestHarness::new().await.unwrap()
        .register_real_hook(permission_hook);
    
    // Set up auto-approval
    let responder = TestPermissionResponder::auto_approve();
    let _action_rx = responder.setup_action_sender(&mut harness);
    
    let args = HookTestHarness::create_command_args("ls", vec!["-la", "/home"]);
    
    let result = harness.execute_tool_with_hooks("execute_command", args).await.unwrap();
    
    // Verify command was executed
    assert!(result.to_string().contains("Executed: ls -la /home"));
    
    // Verify command was tracked in in-memory executor
    let executions = harness.get_executed_commands();
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].0, "ls");
    assert_eq!(executions[0].1, vec!["la", "/home"]);
}

#[tokio::test]
async fn test_multiple_hooks_execution_order() {
    // Test that multiple real hooks execute in the correct order
    let hook1 = Arc::new(PermissionHook::from_config(
        PermissionHookConfig {
            enabled: true,
            require_approval: vec![],
            always_allow: vec![ToolPattern::Pattern { pattern: "write_*".to_string() }],
            always_deny: vec![],
            custom_messages: Default::default(),
        },
        DefaultBehavior::Deny,
    ));
    
    let hook2 = Arc::new(PermissionHook::from_config(
        PermissionHookConfig {
            enabled: true,
            require_approval: vec![ToolPattern::Pattern { pattern: "write_*".to_string() }],
            always_allow: vec![],
            always_deny: vec![],
            custom_messages: Default::default(),
        },
        DefaultBehavior::Allow,
    ));
    
    let mut harness = HookTestHarness::new().await.unwrap()
        .register_real_hook(hook1)  // This allows write_file
        .register_real_hook(hook2); // This requires approval for write_file
    
    // Set up auto-approval
    let responder = TestPermissionResponder::auto_approve();
    let _action_rx = responder.setup_action_sender(&mut harness);
    
    let args = HookTestHarness::create_file_write_args("/test_multiple_hooks.txt", "test content");
    
    let result = harness.execute_tool_with_hooks("write_file", args).await.unwrap();
    
    // Should succeed because user approved
    assert!(result.to_string().contains("Successfully wrote"));
    
    // Verify file was created in in-memory filesystem
    let content = harness.get_file_content("/test_multiple_hooks.txt").unwrap();
    assert_eq!(content, "test content");
}
```

### 3. UI Component Tests (`tests/permission_prompt_ui.rs`)

Test the permission prompt UI component:

```rust
use aether::components::{Component, PermissionPrompt};
use aether::hooks::{HookResultContext, FileOperation};
use aether::action::{Action, PermissionResponse};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tokio::sync::oneshot;

#[test]
fn test_permission_prompt_rendering() {
    let (tx, _rx) = oneshot::channel();
    let context = Some(HookResultContext::FileModification {
        path: "/test/file.txt".to_string(),
        diff: "--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new".to_string(),
        operation: FileOperation::Modify,
    });
    
    let mut prompt = PermissionPrompt::new(
        "write_file".to_string(),
        "Allow file modification?".to_string(),
        context,
        tx,
    );
    
    // Create a test buffer
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    
    // Render the prompt
    prompt.render(area, &mut buf);
    
    // Verify key elements are rendered
    let content = buf_to_string(&buf);
    assert!(content.contains("write_file"));
    assert!(content.contains("Allow file modification?"));
    assert!(content.contains("MODIFY"));
}

#[tokio::test]
async fn test_permission_prompt_approval() {
    let (tx, rx) = oneshot::channel();
    let mut prompt = PermissionPrompt::new(
        "test_tool".to_string(),
        "Allow?".to_string(),
        None,
        tx,
    );
    
    // Simulate pressing 'y' for approval
    let action = prompt
        .handle_key_event(KeyEvent::from(KeyCode::Char('y')))
        .unwrap();
    
    assert_eq!(action, Some(Action::DismissPermissionPrompt));
    
    // Verify approval was sent
    let response = rx.await.unwrap();
    assert!(matches!(response, PermissionResponse::Approved));
}

#[tokio::test]
async fn test_permission_prompt_feedback() {
    let (tx, rx) = oneshot::channel();
    let mut prompt = PermissionPrompt::new(
        "test_tool".to_string(),
        "Allow?".to_string(),
        None,
        tx,
    );
    
    // Press 'f' to enter feedback mode
    let action = prompt
        .handle_key_event(KeyEvent::from(KeyCode::Char('f')))
        .unwrap();
    assert_eq!(action, None);
    
    // Type feedback
    for ch in "Not safe".chars() {
        prompt
            .handle_key_event(KeyEvent::from(KeyCode::Char(ch)))
            .unwrap();
    }
    
    // Submit feedback
    let action = prompt
        .handle_key_event(KeyEvent::from(KeyCode::Enter))
        .unwrap();
    assert_eq!(action, Some(Action::DismissPermissionPrompt));
    
    // Verify feedback was sent
    let response = rx.await.unwrap();
    assert!(matches!(
        response,
        PermissionResponse::DeniedWithFeedback(msg) if msg == "Not safe"
    ));
}

fn buf_to_string(buf: &Buffer) -> String {
    let mut result = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            result.push(buf.get(x, y).symbol.chars().next().unwrap_or(' '));
        }
        result.push('\n');
    }
    result
}
```

### 4. Benchmark Tests (`benches/hook_performance.rs`)

Add performance benchmarks:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use aether::hooks::{HookManager, PermissionHook, HookContext};
use std::sync::Arc;

fn benchmark_hook_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("single_hook_execution", |b| {
        let hook = Arc::new(PermissionHook::default());
        let mut manager = HookManager::new();
        manager.register(hook);
        
        let context = HookContext::new(
            "test_tool".to_string(),
            "test_server".to_string(),
            serde_json::json!({}),
            "test_session".to_string(),
        );
        
        b.iter(|| {
            rt.block_on(async {
                black_box(manager.run_pre_hooks(&context).await.unwrap())
            })
        });
    });
    
    c.bench_function("multiple_hooks_execution", |b| {
        let mut manager = HookManager::new();
        
        // Register 10 hooks
        for i in 0..10 {
            manager.register(Arc::new(PermissionHook::default()));
        }
        
        let context = HookContext::new(
            "test_tool".to_string(),
            "test_server".to_string(),
            serde_json::json!({}),
            "test_session".to_string(),
        );
        
        b.iter(|| {
            rt.block_on(async {
                black_box(manager.run_pre_hooks(&context).await.unwrap())
            })
        });
    });
}

criterion_group!(benches, benchmark_hook_execution);
criterion_main!(benches);
```

### 5. Test Configuration

Update `Cargo.toml` to include test dependencies:

```toml
[dev-dependencies]
tokio-test = "0.4"
tempfile = "3.8"
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "hook_performance"
harness = false

[profile.test]
opt-level = 1  # Faster test compilation
```

## Testing Requirements

### Test Coverage Goals

1. **Unit Tests** (>90% coverage):
   - All hook trait implementations
   - Configuration parsing and validation
   - Pattern matching logic
   - Diff generation

2. **Integration Tests**:
   - End-to-end hook execution flow
   - Permission prompt UI interaction
   - Configuration loading from files
   - Multi-hook scenarios

3. **Performance Tests**:
   - Hook execution overhead < 10ms
   - Multiple hooks scale linearly
   - No memory leaks in long-running scenarios

### CI/CD Integration

Add to `.github/workflows/test.yml`:

```yaml
- name: Run hook tests
  run: |
    cargo test --test hook_integration
    cargo test --test permission_prompt_ui
    
- name: Run benchmarks
  run: cargo bench --bench hook_performance
```

## Acceptance Criteria

- [ ] Fake MCP server with in-memory transport works for testing
- [ ] Real hooks can be tested against fake tools
- [ ] Test harness simplifies end-to-end hook testing
- [ ] File operation fixtures handle temp files correctly
- [ ] Integration tests cover all major scenarios with real implementations
- [ ] UI tests verify rendering and interaction
- [ ] Performance benchmarks establish baselines
- [ ] Test utilities are well-documented
- [ ] All tests pass reliably
- [ ] Coverage reports show >90% for hook modules
- [ ] Tests verify actual file system operations
- [ ] Tests verify real diff generation

## Notes for Implementation

- Use `tokio::test` for async test functions
- Fake MCP servers should implement real MCP protocol behavior
- Real hooks should be tested, not mocked, to verify actual behavior
- Test both success and error paths with real file operations
- Include edge cases like empty diffs, missing files, permission errors
- Performance tests should use realistic workloads with real tools
- Use in-memory transport to avoid network dependencies
- Document test utilities with examples showing real usage
- Make sure tests clean up temp files properly
- Tests should verify that files are actually written/read, not just that methods were called
- Diff generation should be tested with real file content changes
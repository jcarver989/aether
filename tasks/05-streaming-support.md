# Task 5: Add Streaming Support with Tauri Channels

## Overview
Implement real-time streaming communication between the Rust backend and React frontend using Tauri Channels. This enables live updates during LLM response generation and tool execution.

## Goals
- Implement Rust-side streaming with aether_core integration
- Set up channel handlers for different stream types
- Add real-time UI updates during streaming
- Handle concurrent streams (multiple tool calls)
- Implement proper error handling and recovery
- Ensure smooth user experience with loading states

## Steps

### 5.1 Implement Rust Streaming Commands
**File**: `packages/aether-desktop/src-tauri/src/commands/chat.rs` (update)

```rust
use tauri::{ipc::Channel, State};
use aether_core::{
    agent::Agent,
    llm::{LlmProvider, StreamChunk},
    types::ChatMessage as CoreChatMessage,
};
use crate::state::AgentState;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use uuid::Uuid;

#[derive(Clone, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamEvent {
    Start { message_id: String },
    Content { chunk: String },
    ToolCallStart { id: String, name: String },
    ToolCallArgument { id: String, chunk: String },
    ToolCallComplete { id: String },
    Done,
    Error { message: String },
}

#[tauri::command]
#[specta::specta]
pub async fn send_message(
    content: String,
    on_stream: Channel<StreamEvent>,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let message_id = Uuid::new_v4().to_string();
    
    // Send start event
    on_stream.send(StreamEvent::Start { 
        message_id: message_id.clone() 
    }).await
    .map_err(|e| format!("Failed to send start event: {}", e))?;

    // Get agent from state
    let agent_guard = state.agent.lock().await;
    let agent = agent_guard.as_ref()
        .ok_or("Agent not initialized")?;

    // Add user message to agent
    let user_message = CoreChatMessage::User {
        content: content.clone(),
        timestamp: std::time::SystemTime::now(),
    };
    
    // Create a mutable clone for streaming
    // Note: This is a simplified approach - in practice you'd want to
    // manage agent state more carefully
    drop(agent_guard);
    let mut agent_guard = state.agent.lock().await;
    if let Some(ref mut agent) = agent_guard.as_mut() {
        agent.add_message(user_message);
        
        // Start streaming response
        match agent.stream_response().await {
            Ok(mut stream) => {
                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            match chunk {
                                StreamChunk::Content(text) => {
                                    if let Err(e) = on_stream.send(StreamEvent::Content { 
                                        chunk: text 
                                    }).await {
                                        eprintln!("Failed to send content chunk: {}", e);
                                        break;
                                    }
                                }
                                StreamChunk::ToolCallStart { id, name } => {
                                    if let Err(e) = on_stream.send(StreamEvent::ToolCallStart { 
                                        id, name 
                                    }).await {
                                        eprintln!("Failed to send tool call start: {}", e);
                                        break;
                                    }
                                }
                                StreamChunk::ToolCallArgument { id, argument } => {
                                    if let Err(e) = on_stream.send(StreamEvent::ToolCallArgument { 
                                        id, chunk: argument 
                                    }).await {
                                        eprintln!("Failed to send tool call argument: {}", e);
                                        break;
                                    }
                                }
                                StreamChunk::ToolCallComplete { id } => {
                                    if let Err(e) = on_stream.send(StreamEvent::ToolCallComplete { 
                                        id 
                                    }).await {
                                        eprintln!("Failed to send tool call complete: {}", e);
                                        break;
                                    }
                                }
                                StreamChunk::Done => {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = on_stream.send(StreamEvent::Error { 
                                message: e.to_string() 
                            }).await;
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = on_stream.send(StreamEvent::Error { 
                    message: e.to_string() 
                }).await;
                return Err(e.to_string());
            }
        }
    }

    // Send completion event
    on_stream.send(StreamEvent::Done).await
        .map_err(|e| format!("Failed to send done event: {}", e))?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn execute_tool_call(
    tool_id: String,
    tool_name: String,
    tool_params: serde_json::Value,
    on_stream: Channel<StreamEvent>,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let agent_guard = state.agent.lock().await;
    let agent = agent_guard.as_ref()
        .ok_or("Agent not initialized")?;

    // Send tool call start
    on_stream.send(StreamEvent::ToolCallStart { 
        id: tool_id.clone(), 
        name: tool_name.clone() 
    }).await
    .map_err(|e| format!("Failed to send tool call start: {}", e))?;

    // Execute tool through agent
    match agent.execute_tool(&tool_name, tool_params).await {
        Ok(result) => {
            // Send completion
            on_stream.send(StreamEvent::ToolCallComplete { 
                id: tool_id 
            }).await
            .map_err(|e| format!("Failed to send tool call complete: {}", e))?;
            
            Ok(result)
        }
        Err(e) => {
            // Send error
            on_stream.send(StreamEvent::Error { 
                message: e.to_string() 
            }).await
            .map_err(|_| "Failed to send error event")?;
            
            Err(e.to_string())
        }
    }
}
```

### 5.2 Add Agent Initialization Command
**File**: `packages/aether-desktop/src-tauri/src/commands/config.rs`

```rust
use tauri::State;
use aether_core::{
    agent::Agent,
    llm::{openrouter::OpenRouterProvider, ollama::OllamaProvider},
    tools::ToolRegistry,
};
use crate::state::AgentState;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, specta::Type)]
pub struct ProviderConfig {
    pub provider_type: String, // "openrouter" or "ollama"
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: String,
}

#[tauri::command]
#[specta::specta]
pub async fn initialize_agent(
    config: ProviderConfig,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let provider: Box<dyn LlmProvider> = match config.provider_type.as_str() {
        "openrouter" => {
            let api_key = config.api_key
                .ok_or("API key required for OpenRouter")?;
            Box::new(OpenRouterProvider::new(api_key, config.model))
        }
        "ollama" => {
            let base_url = config.base_url
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            Box::new(OllamaProvider::new(base_url, config.model))
        }
        _ => return Err(format!("Unknown provider type: {}", config.provider_type)),
    };

    let tool_registry = {
        let registry_guard = state.tool_registry.lock().await;
        registry_guard.clone()
    };

    let agent = Agent::new(
        provider,
        tool_registry,
        Some("You are a helpful AI assistant.".to_string()),
    );

    let mut agent_guard = state.agent.lock().await;
    *agent_guard = Some(agent);

    Ok(())
}

#[derive(Serialize, specta::Type)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn test_provider_connection(
    config: ProviderConfig,
) -> Result<ConnectionStatus, String> {
    // Test connection logic here
    // For now, just return success
    Ok(ConnectionStatus {
        connected: true,
        error: None,
    })
}
```

### 5.3 Update Tauri Command Registration
**File**: `packages/aether-desktop/src-tauri/src/lib.rs` (update)

```rust
mod state;
mod commands;

use state::AgentState;
use commands::{chat::*, config::*};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Generate TypeScript types in debug mode
    #[cfg(debug_assertions)]
    {
        use specta_typescript::Typescript;
        use tauri_specta::{collect_commands, collect_events, Builder};

        let builder = Builder::<tauri::Wry>::new()
            .commands(collect_commands![
                send_message,
                execute_tool_call,
                initialize_agent,
                test_provider_connection,
            ])
            .events(collect_events![StreamEvent]);

        builder
            .export(
                Typescript::default(),
                "../src/generated/invoke.ts",
            )
            .expect("Failed to export typescript types");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AgentState::default())
        .invoke_handler(tauri::generate_handler![
            send_message,
            execute_tool_call,
            initialize_agent,
            test_provider_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 5.4 Update ChatActions with Real Streaming
**File**: `packages/aether-desktop/src/state/actions/chat.ts` (update key methods)

```typescript
import { commands } from "@/generated/invoke";
// ... other imports

export class ChatActions extends BaseActions {
  // ... existing methods

  async sendMessage(content: string): Promise<void> {
    const userMessageId = crypto.randomUUID();
    const assistantMessageId = crypto.randomUUID();

    try {
      // Add user message block
      this.addUserMessage(userMessageId, content);
      
      // Create streaming assistant message
      this.startStreamingMessage(assistantMessageId);
      
      // Set up streaming channel
      const channel = this.createChannel<StreamEvent>((event) => {
        this.handleStreamEvent(event, assistantMessageId);
      });
      
      // Invoke Tauri command with streaming
      await this.tauriCommands.sendMessage(content, channel);
      
    } catch (error) {
      this.handleStreamError(error as Error, assistantMessageId);
    }
  }

  async executeToolCall(
    toolId: string, 
    toolName: string, 
    toolParams: any
  ): Promise<void> {
    // Update tool state to running
    this.setState((state: AppState) => ({
      ...state,
      toolCalls: new Map(state.toolCalls).set(toolId, 'running'),
    }));

    try {
      const channel = this.createChannel<StreamEvent>((event) => {
        this.handleToolStreamEvent(event, toolId);
      });

      const result = await this.tauriCommands.executeToolCall(
        toolId,
        toolName,
        toolParams,
        channel
      );

      // Add tool result message
      this.addToolResult(toolId, result, true);
      
    } catch (error) {
      this.setState((state: AppState) => ({
        ...state,
        toolCalls: new Map(state.toolCalls).set(toolId, 'failed'),
      }));

      this.addToolResult(toolId, (error as Error).message, false);
    }
  }

  private handleToolStreamEvent(event: StreamEvent, toolId: string): void {
    switch (event.type) {
      case 'toolCallStart':
        // Already handled in executeToolCall
        break;
        
      case 'toolCallComplete':
        this.setState((state: AppState) => ({
          ...state,
          toolCalls: new Map(state.toolCalls).set(toolId, 'completed'),
        }));
        break;
        
      case 'error':
        this.setState((state: AppState) => ({
          ...state,
          toolCalls: new Map(state.toolCalls).set(toolId, 'failed'),
        }));
        break;
    }
  }

  private addToolResult(
    toolCallId: string, 
    content: string, 
    success: boolean
  ): void {
    const toolResult: ChatMessageBlock = {
      id: crypto.randomUUID(),
      message: {
        type: 'tool_result',
        toolCallId,
        content,
        timestamp: new Date(),
        success,
      },
    };

    this.setState((state: AppState) => ({
      ...state,
      messages: [...state.messages, toolResult],
    }));
  }

  // ... rest of existing methods
}
```

### 5.5 Update ConfigActions with Real Commands
**File**: `packages/aether-desktop/src/state/actions/config.ts` (update key methods)

```typescript
export class ConfigActions extends BaseActions {
  // ... existing methods

  async selectProvider(provider: LlmProvider): Promise<void> {
    this.setState((state: AppState) => ({
      ...state,
      activeProvider: provider,
      connectionStatus: {
        ...state.connectionStatus,
        provider: { connected: false },
      },
    }));

    try {
      const config = this.getState().providerConfigs[provider];
      
      // Initialize agent with new provider
      await this.tauriCommands.initializeAgent({
        providerType: provider,
        apiKey: 'apiKey' in config ? config.apiKey : undefined,
        baseUrl: 'baseUrl' in config ? config.baseUrl : undefined,
        model: config.model,
      });
      
      this.setState((state: AppState) => ({
        ...state,
        connectionStatus: {
          ...state.connectionStatus,
          provider: { connected: true },
        },
      }));
    } catch (error) {
      this.setState((state: AppState) => ({
        ...state,
        connectionStatus: {
          ...state.connectionStatus,
          provider: { 
            connected: false, 
            error: (error as Error).message 
          },
        },
      }));
    }
  }

  private async testProviderConnection(provider: LlmProvider): Promise<void> {
    const config = this.getState().providerConfigs[provider];
    
    const result = await this.tauriCommands.testProviderConnection({
      providerType: provider,
      apiKey: 'apiKey' in config ? config.apiKey : undefined,
      baseUrl: 'baseUrl' in config ? config.baseUrl : undefined,
      model: config.model,
    });

    if (!result.connected) {
      throw new Error(result.error || 'Connection failed');
    }
  }

  // ... rest of existing methods
}
```

### 5.6 Create Message List Component with Streaming
**File**: `packages/aether-desktop/src/components/chat/MessageList.tsx`

```tsx
import React, { useEffect, useRef } from 'react';
import { MessageBlock } from './MessageBlock';
import { useSelector } from '@/hooks/useSelector';
import { cn } from '@/lib/utils';

interface MessageListProps {
  className?: string;
}

export const MessageList: React.FC<MessageListProps> = ({ className }) => {
  const messages = useSelector(state => state.messages);
  const streamingMessage = useSelector(state => state.streamingMessage);
  const autoScroll = useSelector(state => state.scroll.autoScroll);
  
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    if (autoScroll && messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ 
        behavior: 'smooth',
        block: 'end'
      });
    }
  }, [messages.length, streamingMessage?.partialContent, autoScroll]);

  const handleCopy = async (content: string) => {
    try {
      await navigator.clipboard.writeText(content);
      // TODO: Show toast notification
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
    }
  };

  return (
    <div 
      ref={containerRef}
      className={cn(
        "flex-1 overflow-y-auto p-4 space-y-3",
        className
      )}
    >
      {messages.map((message) => (
        <MessageBlock
          key={message.id}
          block={message}
          onCopy={handleCopy}
        />
      ))}
      
      {streamingMessage && (
        <MessageBlock
          block={streamingMessage}
          onCopy={handleCopy}
        />
      )}
      
      <div ref={messagesEndRef} />
    </div>
  );
};
```

### 5.7 Create Chat Input Component
**File**: `packages/aether-desktop/src/components/input/ChatInput.tsx`

```tsx
import React, { useState, useRef, KeyboardEvent } from 'react';
import { Send, Square } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { useAppContext } from '@/hooks/useAppContext';
import { useSelector } from '@/hooks/useSelector';
import { cn } from '@/lib/utils';

interface ChatInputProps {
  className?: string;
}

export const ChatInput: React.FC<ChatInputProps> = ({ className }) => {
  const [input, setInput] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { actions } = useAppContext();
  
  const isStreaming = useSelector(state => state.streamingMessage !== null);

  const handleSubmit = async () => {
    if (!input.trim() || isStreaming) return;

    const message = input.trim();
    setInput('');
    
    try {
      await actions.chat.sendMessage(message);
    } catch (error) {
      console.error('Failed to send message:', error);
      // TODO: Show error toast
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleStop = () => {
    // TODO: Implement stream cancellation
    console.log('Stop streaming requested');
  };

  // Auto-resize textarea
  React.useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [input]);

  return (
    <div className={cn(
      "border-t bg-background p-4",
      className
    )}>
      <div className="flex gap-2 items-end">
        <div className="flex-1">
          <Textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type your message... (Enter to send, Shift+Enter for new line)"
            className="min-h-[40px] max-h-[200px] resize-none"
            disabled={isStreaming}
          />
        </div>
        
        <Button
          onClick={isStreaming ? handleStop : handleSubmit}
          disabled={!input.trim() && !isStreaming}
          size="sm"
          variant={isStreaming ? "destructive" : "default"}
        >
          {isStreaming ? (
            <>
              <Square className="h-4 w-4 mr-1" />
              Stop
            </>
          ) : (
            <>
              <Send className="h-4 w-4 mr-1" />
              Send
            </>
          )}
        </Button>
      </div>
    </div>
  );
};
```

### 5.8 Create Main Chat View
**File**: `packages/aether-desktop/src/components/chat/ChatView.tsx`

```tsx
import React from 'react';
import { MessageList } from './MessageList';
import { ChatInput } from '../input/ChatInput';
import { cn } from '@/lib/utils';

interface ChatViewProps {
  className?: string;
}

export const ChatView: React.FC<ChatViewProps> = ({ className }) => {
  return (
    <div className={cn(
      "flex flex-col h-full bg-background",
      className
    )}>
      <div className="flex-1 min-h-0">
        <MessageList />
      </div>
      
      <ChatInput />
    </div>
  );
};
```

## Testing

### 5.9 Create Streaming Tests
**File**: `packages/aether-desktop/src/state/actions/__tests__/streaming.test.ts`

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ChatActions } from '../chat';
import { createStore } from '../../store';
import { createMockChannel, MockChannel } from '../../../tests/mocks/mockChannel';
import { StreamEvent } from '@/types';

describe('Streaming Integration', () => {
  let store: ReturnType<typeof createStore>;
  let mockCommands: any;
  let chatActions: ChatActions;
  let capturedChannel: MockChannel<StreamEvent> | null = null;

  beforeEach(() => {
    store = createStore();
    mockCommands = {
      sendMessage: vi.fn(),
      executeToolCall: vi.fn(),
    };
    
    const createChannel = (onMessage: (message: StreamEvent) => void) => {
      capturedChannel = createMockChannel(onMessage);
      return capturedChannel;
    };

    chatActions = new ChatActions(store, mockCommands, createChannel);
  });

  it('should handle complex streaming scenario with tool calls', async () => {
    mockCommands.sendMessage.mockImplementation(async () => {
      if (capturedChannel) {
        await capturedChannel.simulateStream([
          { type: 'start', messageId: 'msg-1' },
          { type: 'content', chunk: 'I need to call a tool.' },
          { type: 'toolCallStart', id: 'tool-1', name: 'search' },
          { type: 'toolCallArgument', id: 'tool-1', chunk: '{"query": "' },
          { type: 'toolCallArgument', id: 'tool-1', chunk: 'test query"}' },
          { type: 'toolCallComplete', id: 'tool-1' },
          { type: 'content', chunk: ' Based on the results...' },
          { type: 'done' },
        ] as StreamEvent[]);
      }
    });

    await chatActions.sendMessage('Test complex message');
    await new Promise(resolve => setTimeout(resolve, 200));

    const state = store.getState();
    
    // Should have user message, tool call, and final assistant message
    expect(state.messages.length).toBeGreaterThanOrEqual(2);
    expect(state.toolCalls.get('tool-1')).toBe('completed');
    expect(state.streamingMessage).toBeNull();
  });

  it('should handle streaming errors gracefully', async () => {
    mockCommands.sendMessage.mockImplementation(async () => {
      if (capturedChannel) {
        await capturedChannel.simulateStream([
          { type: 'start', messageId: 'msg-1' },
          { type: 'content', chunk: 'Starting response...' },
          { type: 'error', message: 'Connection lost' },
        ] as StreamEvent[]);
      }
    });

    await chatActions.sendMessage('Test error handling');
    await new Promise(resolve => setTimeout(resolve, 100));

    const state = store.getState();
    
    // Should have error message
    const errorMessage = state.messages.find(m => m.message.type === 'error');
    expect(errorMessage).toBeDefined();
    expect(errorMessage?.message.message).toBe('Connection lost');
  });
});
```

## Acceptance Criteria
- [ ] Real-time streaming from Rust backend to React frontend
- [ ] Smooth UI updates during message streaming
- [ ] Concurrent tool call execution and streaming
- [ ] Proper error handling and recovery
- [ ] Stream cancellation capability
- [ ] Auto-scroll behavior during streaming
- [ ] Loading states and animations
- [ ] Channel cleanup on component unmount
- [ ] Type-safe communication between Rust and TypeScript
- [ ] Comprehensive test coverage for streaming scenarios

## Dependencies
- Task 1: Setup Foundation (Tauri backend structure)
- Task 2: TypeScript Types (event interfaces)
- Task 3: Action Classes (state management)
- Task 4: Block UI Components (visual components)

## Next Steps
After completing this task, proceed to:
- Task 6: Create testing infrastructure with mock channels
- Additional features: Virtual scrolling, markdown rendering, keyboard shortcuts
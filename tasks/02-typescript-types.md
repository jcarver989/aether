# Task 2: Create TypeScript Types for Chat Messages and Events

## Overview
Establish comprehensive TypeScript type definitions that mirror the aether_core Rust types, focusing on chat messages, streaming events, and configuration types.

## Goals
- Create type-safe interfaces for all chat-related data structures
- Define streaming event types for channel communication
- Set up configuration types for providers and MCP servers
- Ensure types are generated from Rust where possible

## Steps

### 2.1 Create Core Chat Types
**File**: `packages/aether-desktop/src/types/chat.ts`

```typescript
export interface ChatMessageBlock {
  id: string;
  message: ChatMessage;
  height?: number;
  collapsed?: boolean;
}

export interface StreamingMessageBlock extends ChatMessageBlock {
  isStreaming: boolean;
  partialContent: string;
}

export type ChatMessage = 
  | SystemMessage
  | UserMessage  
  | AssistantMessage
  | ToolCallMessage
  | ToolResultMessage
  | ErrorMessage;

export interface SystemMessage {
  type: 'system';
  content: string;
  timestamp: Date;
}

export interface UserMessage {
  type: 'user';
  content: string;
  timestamp: Date;
}

export interface AssistantMessage {
  type: 'assistant';
  content: string;
  timestamp: Date;
}

export interface ToolCallMessage {
  type: 'tool_call';
  id: string;
  name: string;
  params: Record<string, any>;
  timestamp: Date;
}

export interface ToolResultMessage {
  type: 'tool_result';
  toolCallId: string;
  content: string;
  timestamp: Date;
  success?: boolean;
}

export interface ErrorMessage {
  type: 'error';
  message: string;
  timestamp: Date;
  source?: 'agent' | 'tool' | 'system';
}

export type ToolCallState = 'pending' | 'running' | 'completed' | 'failed';
```

### 2.2 Create Streaming Event Types
**File**: `packages/aether-desktop/src/types/events.ts`

```typescript
export type StreamEvent = 
  | StreamStartEvent
  | StreamContentEvent
  | StreamToolCallStartEvent
  | StreamToolCallArgumentEvent
  | StreamToolCallCompleteEvent
  | StreamDoneEvent
  | StreamErrorEvent;

export interface StreamStartEvent {
  type: 'start';
  messageId: string;
}

export interface StreamContentEvent {
  type: 'content';
  chunk: string;
}

export interface StreamToolCallStartEvent {
  type: 'toolCallStart';
  id: string;
  name: string;
}

export interface StreamToolCallArgumentEvent {
  type: 'toolCallArgument';
  id: string;
  chunk: string;
}

export interface StreamToolCallCompleteEvent {
  type: 'toolCallComplete';
  id: string;
}

export interface StreamDoneEvent {
  type: 'done';
}

export interface StreamErrorEvent {
  type: 'error';
  message: string;
}

// Tool discovery events
export type ToolDiscoveryEvent = 
  | ToolDiscoveredEvent
  | ToolDiscoveryCompleteEvent
  | ToolDiscoveryErrorEvent;

export interface ToolDiscoveredEvent {
  type: 'discovered';
  tool: ToolDefinition;
}

export interface ToolDiscoveryCompleteEvent {
  type: 'complete';
  count: number;
}

export interface ToolDiscoveryErrorEvent {
  type: 'error';
  message: string;
}
```

### 2.3 Create Configuration Types
**File**: `packages/aether-desktop/src/types/config.ts`

```typescript
export type LlmProvider = 'openrouter' | 'ollama';

export interface ProviderConfig {
  openrouter: OpenRouterConfig;
  ollama: OllamaConfig;
}

export interface OpenRouterConfig {
  apiKey: string;
  model: string;
  baseUrl?: string;
  temperature?: number;
}

export interface OllamaConfig {
  baseUrl: string;
  model: string;
  temperature?: number;
}

export interface McpServerConfig {
  id: string;
  name: string;
  type: 'http' | 'stdio';
  enabled: boolean;
  config: HttpMcpConfig | StdioMcpConfig;
}

export interface HttpMcpConfig {
  url: string;
  headers?: Record<string, string>;
}

export interface StdioMcpConfig {
  command: string;
  args: string[];
  env?: Record<string, string>;
}

export interface ToolDefinition {
  name: string;
  description: string;
  parameters: Record<string, any>; // JSON Schema
  server?: string; // Which MCP server provides this tool
}

export interface ConnectionStatus {
  provider: {
    connected: boolean;
    error?: string;
  };
  mcpServers: Record<string, {
    connected: boolean;
    error?: string;
    toolCount: number;
  }>;
}
```

### 2.4 Create UI State Types
**File**: `packages/aether-desktop/src/types/ui.ts`

```typescript
export type Theme = 'light' | 'dark' | 'system';

export interface UIState {
  theme: Theme;
  sidebarOpen: boolean;
  settingsOpen: boolean;
  commandPaletteOpen: boolean;
}

export interface ScrollState {
  offset: number;
  atBottom: boolean;
  autoScroll: boolean;
}

export interface BlockTheme {
  system: string;
  user: string;
  assistant: string;
  toolCall: string;
  toolResult: string;
  error: string;
  streaming: string;
}

export const defaultBlockTheme: BlockTheme = {
  system: 'border-gray-400 bg-gray-50 text-gray-900',
  user: 'border-blue-400 bg-blue-50 text-blue-900',
  assistant: 'border-green-400 bg-green-50 text-green-900',
  toolCall: 'border-purple-400 bg-purple-50 text-purple-900',
  toolResult: 'border-orange-400 bg-orange-50 text-orange-900',
  error: 'border-red-400 bg-red-50 text-red-900',
  streaming: 'animate-pulse-subtle',
};
```

### 2.5 Update AppState with Complete Types
**File**: `packages/aether-desktop/src/state/store.ts` (update)

```typescript
import { create, StoreApi, UseBoundStore } from "zustand";
import { ChatMessageBlock, StreamingMessageBlock, ToolCallState } from "@/types/chat";
import { LlmProvider, ProviderConfig, McpServerConfig, ToolDefinition, ConnectionStatus } from "@/types/config";
import { UIState, ScrollState } from "@/types/ui";

export interface AppState {
  // Chat Domain
  messages: ChatMessageBlock[];
  streamingMessage: StreamingMessageBlock | null;
  toolCalls: Map<string, ToolCallState>;
  
  // Configuration Domain
  activeProvider: LlmProvider;
  providerConfigs: ProviderConfig;
  mcpServers: McpServerConfig[];
  availableTools: ToolDefinition[];
  connectionStatus: ConnectionStatus;
  
  // UI State
  ui: UIState;
  scroll: ScrollState;
  selectedMessageId: string | null;
}

export type ZustandStore<T> = UseBoundStore<StoreApi<T>>;

export function createStore(initialState: AppState = defaultAppState()): ZustandStore<AppState> {
  return create<AppState>(() => initialState);
}

export function defaultAppState(): AppState {
  return {
    // Chat Domain
    messages: [],
    streamingMessage: null,
    toolCalls: new Map(),
    
    // Configuration Domain
    activeProvider: 'openrouter',
    providerConfigs: {
      openrouter: {
        apiKey: '',
        model: 'anthropic/claude-3-5-sonnet',
        temperature: 0.7,
      },
      ollama: {
        baseUrl: 'http://localhost:11434',
        model: 'llama2',
        temperature: 0.7,
      },
    },
    mcpServers: [],
    availableTools: [],
    connectionStatus: {
      provider: { connected: false },
      mcpServers: {},
    },
    
    // UI State
    ui: {
      theme: 'system',
      sidebarOpen: true,
      settingsOpen: false,
      commandPaletteOpen: false,
    },
    scroll: {
      offset: 0,
      atBottom: true,
      autoScroll: true,
    },
    selectedMessageId: null,
  };
}
```

### 2.6 Create Type Guards and Utilities
**File**: `packages/aether-desktop/src/types/guards.ts`

```typescript
import { ChatMessage, StreamEvent, ToolDiscoveryEvent } from "./index";

// Type guards for chat messages
export function isSystemMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'system' }> {
  return message.type === 'system';
}

export function isUserMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'user' }> {
  return message.type === 'user';
}

export function isAssistantMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'assistant' }> {
  return message.type === 'assistant';
}

export function isToolCallMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'tool_call' }> {
  return message.type === 'tool_call';
}

export function isToolResultMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'tool_result' }> {
  return message.type === 'tool_result';
}

export function isErrorMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'error' }> {
  return message.type === 'error';
}

// Type guards for events
export function isStreamContentEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'content' }> {
  return event.type === 'content';
}

export function isStreamErrorEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'error' }> {
  return event.type === 'error';
}

export function isToolDiscoveredEvent(event: ToolDiscoveryEvent): event is Extract<ToolDiscoveryEvent, { type: 'discovered' }> {
  return event.type === 'discovered';
}
```

### 2.7 Create Index File for Types
**File**: `packages/aether-desktop/src/types/index.ts`

```typescript
// Re-export all types from a central location
export * from './chat';
export * from './events';
export * from './config';
export * from './ui';
export * from './guards';

// Utility types
export type AsyncResult<T> = Promise<T | Error>;
export type Optional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;
export type RequiredFields<T, K extends keyof T> = T & Required<Pick<T, K>>;

// Channel type for dependency injection
export interface Channel<T> {
  id: string;
  onmessage?: (message: T) => void;
  send?: (message: T) => Promise<void>;
}

// Factory type for creating channels
export type ChannelFactory = <T>(onMessage: (message: T) => void) => Channel<T>;
```

### 2.8 Update Rust Types to Match
**File**: `packages/aether-desktop/src-tauri/src/commands/chat.rs` (update)

```rust
use specta::Type;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize, Type)]
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

#[derive(Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToolDiscoveryEvent {
    Discovered { tool: ToolDefinition },
    Complete { count: u32 },
    Error { message: String },
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub server: Option<String>,
}
```

## Testing

### 2.9 Create Type Tests
**File**: `packages/aether-desktop/src/types/__tests__/types.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import { 
  isSystemMessage, 
  isUserMessage, 
  isAssistantMessage,
  isStreamContentEvent,
  isStreamErrorEvent 
} from '../guards';
import { ChatMessage, StreamEvent } from '../index';

describe('Type Guards', () => {
  it('should correctly identify message types', () => {
    const systemMessage: ChatMessage = {
      type: 'system',
      content: 'System message',
      timestamp: new Date(),
    };
    
    expect(isSystemMessage(systemMessage)).toBe(true);
    expect(isUserMessage(systemMessage)).toBe(false);
  });

  it('should correctly identify stream events', () => {
    const contentEvent: StreamEvent = {
      type: 'content',
      chunk: 'Hello',
    };
    
    expect(isStreamContentEvent(contentEvent)).toBe(true);
    expect(isStreamErrorEvent(contentEvent)).toBe(false);
  });
});
```

## Acceptance Criteria
- [ ] All chat message types defined with proper TypeScript interfaces
- [ ] Streaming event types match Rust definitions
- [ ] Configuration types support both OpenRouter and Ollama
- [ ] Type guards provide runtime type checking
- [ ] AppState uses proper typed interfaces
- [ ] No TypeScript compilation errors
- [ ] Types are exported from central index file

## Dependencies
- Task 1: Setup Foundation (for basic structure)

## Next Steps
After completing this task, proceed to:
- Task 3: Implement Action classes with channel factory injection
- Task 4: Build block-based UI components
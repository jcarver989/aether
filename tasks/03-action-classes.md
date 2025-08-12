# Task 3: Implement Action Classes with Channel Factory Injection

## Overview
Create testable Action classes that handle all state mutations and external communication. Actions will use dependency injection for channel creation, making them easy to test and maintaining clean separation of concerns.

## Goals
- Implement ChatActions for message handling and streaming
- Create ConfigActions for provider and MCP server management
- Use channel factory injection for testability
- Handle all state mutations through actions
- Implement proper error handling and loading states

## Steps

### 3.1 Create Base Action Structure
**File**: `packages/aether-desktop/src/state/actions/base.ts`

```typescript
import { ZustandStore } from "../store";
import { ChannelFactory } from "@/types";

export abstract class BaseActions {
  constructor(
    protected store: ZustandStore<any>,
    protected createChannel: ChannelFactory,
  ) {}

  protected setState(updater: (state: any) => any) {
    this.store.setState(updater);
  }

  protected getState() {
    return this.store.getState();
  }
}
```

### 3.2 Implement ChatActions
**File**: `packages/aether-desktop/src/state/actions/chat.ts`

```typescript
import { BaseActions } from "./base";
import { commands } from "@/generated/invoke";
import { AppState, ZustandStore } from "../store";
import { 
  StreamEvent, 
  ChatMessage, 
  ChatMessageBlock, 
  StreamingMessageBlock,
  ChannelFactory 
} from "@/types";

export class ChatActions extends BaseActions {
  constructor(
    store: ZustandStore<AppState>,
    private tauriCommands: typeof commands,
    createChannel: ChannelFactory,
  ) {
    super(store, createChannel);
  }

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
      
      // Invoke Tauri command
      await this.tauriCommands.sendMessage(content, channel);
      
    } catch (error) {
      this.handleStreamError(error as Error, assistantMessageId);
    }
  }

  clearConversation(): void {
    this.setState((state: AppState) => ({
      ...state,
      messages: [],
      streamingMessage: null,
      toolCalls: new Map(),
      selectedMessageId: null,
    }));
  }

  toggleMessageCollapse(messageId: string): void {
    this.setState((state: AppState) => ({
      ...state,
      messages: state.messages.map(msg => 
        msg.id === messageId 
          ? { ...msg, collapsed: !msg.collapsed }
          : msg
      ),
    }));
  }

  selectMessage(messageId: string | null): void {
    this.setState((state: AppState) => ({
      ...state,
      selectedMessageId: messageId,
    }));
  }

  setScrollOffset(offset: number): void {
    this.setState((state: AppState) => ({
      ...state,
      scroll: {
        ...state.scroll,
        offset,
        atBottom: offset === 0,
      },
    }));
  }

  private addUserMessage(id: string, content: string): void {
    const userMessage: ChatMessageBlock = {
      id,
      message: {
        type: 'user',
        content,
        timestamp: new Date(),
      },
    };

    this.setState((state: AppState) => ({
      ...state,
      messages: [...state.messages, userMessage],
    }));
  }

  private startStreamingMessage(id: string): void {
    const streamingMessage: StreamingMessageBlock = {
      id,
      message: {
        type: 'assistant',
        content: '',
        timestamp: new Date(),
      },
      isStreaming: true,
      partialContent: '',
    };

    this.setState((state: AppState) => ({
      ...state,
      streamingMessage,
    }));
  }

  private handleStreamEvent(event: StreamEvent, messageId: string): void {
    switch (event.type) {
      case 'start':
        // Already handled in startStreamingMessage
        break;

      case 'content':
        this.appendStreamContent(event.chunk);
        break;

      case 'toolCallStart':
        this.handleToolCallStart(event.id, event.name);
        break;

      case 'toolCallArgument':
        this.appendToolCallArgument(event.id, event.chunk);
        break;

      case 'toolCallComplete':
        this.completeToolCall(event.id);
        break;

      case 'done':
        this.finalizeStreamingMessage(messageId);
        break;

      case 'error':
        this.handleStreamError(new Error(event.message), messageId);
        break;
    }
  }

  private appendStreamContent(chunk: string): void {
    this.setState((state: AppState) => ({
      ...state,
      streamingMessage: state.streamingMessage ? {
        ...state.streamingMessage,
        partialContent: state.streamingMessage.partialContent + chunk,
      } : null,
    }));
  }

  private handleToolCallStart(toolId: string, toolName: string): void {
    // Add tool call message
    const toolCallMessage: ChatMessageBlock = {
      id: toolId,
      message: {
        type: 'tool_call',
        id: toolId,
        name: toolName,
        params: {},
        timestamp: new Date(),
      },
    };

    this.setState((state: AppState) => ({
      ...state,
      messages: [...state.messages, toolCallMessage],
      toolCalls: new Map(state.toolCalls).set(toolId, 'running'),
    }));
  }

  private appendToolCallArgument(toolId: string, chunk: string): void {
    this.setState((state: AppState) => {
      const updatedMessages = state.messages.map(msg => {
        if (msg.id === toolId && msg.message.type === 'tool_call') {
          return {
            ...msg,
            message: {
              ...msg.message,
              params: {
                ...msg.message.params,
                // Accumulate argument chunks (this is simplified)
                arguments: (msg.message.params.arguments || '') + chunk,
              },
            },
          };
        }
        return msg;
      });

      return {
        ...state,
        messages: updatedMessages,
      };
    });
  }

  private completeToolCall(toolId: string): void {
    this.setState((state: AppState) => ({
      ...state,
      toolCalls: new Map(state.toolCalls).set(toolId, 'completed'),
    }));
  }

  private finalizeStreamingMessage(messageId: string): void {
    this.setState((state: AppState) => {
      if (!state.streamingMessage) return state;

      const finalMessage: ChatMessageBlock = {
        id: messageId,
        message: {
          type: 'assistant',
          content: state.streamingMessage.partialContent,
          timestamp: state.streamingMessage.message.timestamp,
        },
      };

      return {
        ...state,
        messages: [...state.messages, finalMessage],
        streamingMessage: null,
      };
    });
  }

  private handleStreamError(error: Error, messageId: string): void {
    console.error('Stream error:', error);
    
    // Add error message
    const errorMessage: ChatMessageBlock = {
      id: crypto.randomUUID(),
      message: {
        type: 'error',
        message: error.message,
        timestamp: new Date(),
        source: 'agent',
      },
    };

    this.setState((state: AppState) => ({
      ...state,
      messages: [...state.messages, errorMessage],
      streamingMessage: null,
    }));
  }
}
```

### 3.3 Implement ConfigActions
**File**: `packages/aether-desktop/src/state/actions/config.ts`

```typescript
import { BaseActions } from "./base";
import { commands } from "@/generated/invoke";
import { AppState, ZustandStore } from "../store";
import { 
  ToolDiscoveryEvent, 
  LlmProvider, 
  McpServerConfig, 
  ToolDefinition,
  ChannelFactory 
} from "@/types";

export class ConfigActions extends BaseActions {
  constructor(
    store: ZustandStore<AppState>,
    private tauriCommands: typeof commands,
    createChannel: ChannelFactory,
  ) {
    super(store, createChannel);
  }

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
      // Test connection to new provider
      await this.testProviderConnection(provider);
      
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

  updateProviderConfig(provider: LlmProvider, config: any): void {
    this.setState((state: AppState) => ({
      ...state,
      providerConfigs: {
        ...state.providerConfigs,
        [provider]: {
          ...state.providerConfigs[provider],
          ...config,
        },
      },
    }));
  }

  async addMcpServer(config: Omit<McpServerConfig, 'id'>): Promise<void> {
    const id = crypto.randomUUID();
    const serverConfig: McpServerConfig = {
      ...config,
      id,
    };

    this.setState((state: AppState) => ({
      ...state,
      mcpServers: [...state.mcpServers, serverConfig],
      connectionStatus: {
        ...state.connectionStatus,
        mcpServers: {
          ...state.connectionStatus.mcpServers,
          [id]: { connected: false, toolCount: 0 },
        },
      },
    }));

    if (serverConfig.enabled) {
      await this.connectMcpServer(id);
    }
  }

  async connectMcpServer(serverId: string): Promise<void> {
    const state = this.getState();
    const server = state.mcpServers.find(s => s.id === serverId);
    
    if (!server) {
      throw new Error(`MCP server ${serverId} not found`);
    }

    try {
      await this.tauriCommands.connectMcpServer(server);
      
      this.setState((state: AppState) => ({
        ...state,
        connectionStatus: {
          ...state.connectionStatus,
          mcpServers: {
            ...state.connectionStatus.mcpServers,
            [serverId]: { connected: true, toolCount: 0 },
          },
        },
      }));

      // Discover tools from this server
      await this.discoverToolsFromServer(serverId);
      
    } catch (error) {
      this.setState((state: AppState) => ({
        ...state,
        connectionStatus: {
          ...state.connectionStatus,
          mcpServers: {
            ...state.connectionStatus.mcpServers,
            [serverId]: { 
              connected: false, 
              toolCount: 0,
              error: (error as Error).message 
            },
          },
        },
      }));
    }
  }

  async disconnectMcpServer(serverId: string): Promise<void> {
    try {
      await this.tauriCommands.disconnectMcpServer(serverId);
      
      this.setState((state: AppState) => ({
        ...state,
        availableTools: state.availableTools.filter(
          tool => tool.server !== serverId
        ),
        connectionStatus: {
          ...state.connectionStatus,
          mcpServers: {
            ...state.connectionStatus.mcpServers,
            [serverId]: { connected: false, toolCount: 0 },
          },
        },
      }));
    } catch (error) {
      console.error(`Failed to disconnect MCP server ${serverId}:`, error);
    }
  }

  removeMcpServer(serverId: string): void {
    this.setState((state: AppState) => ({
      ...state,
      mcpServers: state.mcpServers.filter(s => s.id !== serverId),
      availableTools: state.availableTools.filter(
        tool => tool.server !== serverId
      ),
      connectionStatus: {
        ...state.connectionStatus,
        mcpServers: Object.fromEntries(
          Object.entries(state.connectionStatus.mcpServers)
            .filter(([id]) => id !== serverId)
        ),
      },
    }));
  }

  async discoverAllTools(): Promise<void> {
    const channel = this.createChannel<ToolDiscoveryEvent>((event) => {
      this.handleToolDiscoveryEvent(event);
    });

    try {
      await this.tauriCommands.discoverTools(channel);
    } catch (error) {
      console.error('Tool discovery failed:', error);
    }
  }

  private async testProviderConnection(provider: LlmProvider): Promise<void> {
    // This would be implemented based on your provider testing needs
    await this.tauriCommands.testProviderConnection(provider);
  }

  private async discoverToolsFromServer(serverId: string): Promise<void> {
    const channel = this.createChannel<ToolDiscoveryEvent>((event) => {
      this.handleToolDiscoveryEvent(event, serverId);
    });

    try {
      await this.tauriCommands.discoverToolsFromServer(serverId, channel);
    } catch (error) {
      console.error(`Tool discovery failed for server ${serverId}:`, error);
    }
  }

  private handleToolDiscoveryEvent(event: ToolDiscoveryEvent, serverId?: string): void {
    switch (event.type) {
      case 'discovered':
        this.addDiscoveredTool(event.tool, serverId);
        break;

      case 'complete':
        this.handleToolDiscoveryComplete(event.count, serverId);
        break;

      case 'error':
        console.error('Tool discovery error:', event.message);
        break;
    }
  }

  private addDiscoveredTool(tool: ToolDefinition, serverId?: string): void {
    const toolWithServer = serverId ? { ...tool, server: serverId } : tool;
    
    this.setState((state: AppState) => ({
      ...state,
      availableTools: [...state.availableTools, toolWithServer],
    }));
  }

  private handleToolDiscoveryComplete(count: number, serverId?: string): void {
    if (serverId) {
      this.setState((state: AppState) => ({
        ...state,
        connectionStatus: {
          ...state.connectionStatus,
          mcpServers: {
            ...state.connectionStatus.mcpServers,
            [serverId]: {
              ...state.connectionStatus.mcpServers[serverId],
              toolCount: count,
            },
          },
        },
      }));
    }
    
    console.log(`Tool discovery complete: ${count} tools found`);
  }
}
```

### 3.4 Create UI Actions
**File**: `packages/aether-desktop/src/state/actions/ui.ts`

```typescript
import { BaseActions } from "./base";
import { AppState, ZustandStore } from "../store";
import { Theme, ChannelFactory } from "@/types";

export class UIActions extends BaseActions {
  constructor(
    store: ZustandStore<AppState>,
    createChannel: ChannelFactory,
  ) {
    super(store, createChannel);
  }

  setTheme(theme: Theme): void {
    this.setState((state: AppState) => ({
      ...state,
      ui: {
        ...state.ui,
        theme,
      },
    }));
  }

  toggleSidebar(): void {
    this.setState((state: AppState) => ({
      ...state,
      ui: {
        ...state.ui,
        sidebarOpen: !state.ui.sidebarOpen,
      },
    }));
  }

  openSettings(): void {
    this.setState((state: AppState) => ({
      ...state,
      ui: {
        ...state.ui,
        settingsOpen: true,
      },
    }));
  }

  closeSettings(): void {
    this.setState((state: AppState) => ({
      ...state,
      ui: {
        ...state.ui,
        settingsOpen: false,
      },
    }));
  }

  toggleCommandPalette(): void {
    this.setState((state: AppState) => ({
      ...state,
      ui: {
        ...state.ui,
        commandPaletteOpen: !state.ui.commandPaletteOpen,
      },
    }));
  }

  enableAutoScroll(): void {
    this.setState((state: AppState) => ({
      ...state,
      scroll: {
        ...state.scroll,
        autoScroll: true,
      },
    }));
  }

  disableAutoScroll(): void {
    this.setState((state: AppState) => ({
      ...state,
      scroll: {
        ...state.scroll,
        autoScroll: false,
      },
    }));
  }
}
```

### 3.5 Update Main Actions Export
**File**: `packages/aether-desktop/src/state/actions.ts` (update)

```typescript
import { commands } from "@/generated/invoke";
import { AppState, ZustandStore } from "./store";
import { ChannelFactory } from "@/types";
import { ChatActions } from "./actions/chat";
import { ConfigActions } from "./actions/config";
import { UIActions } from "./actions/ui";

export interface AppActions {
  chat: ChatActions;
  config: ConfigActions;
  ui: UIActions;
}

export function createAppActions(
  store: ZustandStore<AppState>,
  tauriCommands: typeof commands,
  createChannel: ChannelFactory,
): AppActions {
  return {
    chat: new ChatActions(store, tauriCommands, createChannel),
    config: new ConfigActions(store, tauriCommands, createChannel),
    ui: new UIActions(store, createChannel),
  };
}

// Re-export individual action classes
export { ChatActions } from "./actions/chat";
export { ConfigActions } from "./actions/config";
export { UIActions } from "./actions/ui";
```

### 3.6 Update AppContext
**File**: `packages/aether-desktop/src/hooks/useAppContext.ts` (update)

```typescript
import { commands } from "@/generated/invoke";
import { AppState, ZustandStore } from "@/state/store";
import { ChannelFactory } from "@/types";
import { createContext, useContext } from "react";
import { AppActions } from "@/state/actions";

export interface AppContext {
  commands: typeof commands;
  createChannel: ChannelFactory;
  store: ZustandStore<AppState>;
  actions: AppActions;
}

export const AppContext = createContext<AppContext | null>(null);

export function useAppContext(): AppContext {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error("useAppContext must be used within an AppContextProvider");
  }

  return context;
}
```

## Testing

### 3.7 Create Mock Channel for Testing
**File**: `packages/aether-desktop/src/tests/mocks/mockChannel.ts`

```typescript
import { Channel } from "@/types";

export class MockChannel<T> implements Channel<T> {
  id = crypto.randomUUID();
  onmessage?: (message: T) => void;
  private messages: T[] = [];

  constructor(onMessage?: (message: T) => void) {
    this.onmessage = onMessage;
  }

  async send(message: T): Promise<void> {
    this.messages.push(message);
    // Simulate async behavior
    setTimeout(() => {
      this.onmessage?.(message);
    }, 0);
  }

  // Test helper methods
  async simulateStream(messages: T[], delayMs = 10): Promise<void> {
    for (const message of messages) {
      await this.send(message);
      await new Promise(resolve => setTimeout(resolve, delayMs));
    }
  }

  getMessages(): T[] {
    return [...this.messages];
  }

  clear(): void {
    this.messages = [];
  }
}

export const createMockChannel = <T>(
  onMessage?: (message: T) => void
): MockChannel<T> => {
  return new MockChannel<T>(onMessage);
};
```

### 3.8 Create Action Tests
**File**: `packages/aether-desktop/src/state/actions/__tests__/chat.test.ts`

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ChatActions } from '../chat';
import { createStore } from '../../store';
import { createMockChannel, MockChannel } from '../../../tests/mocks/mockChannel';
import { StreamEvent } from '@/types';

describe('ChatActions', () => {
  let store: ReturnType<typeof createStore>;
  let mockCommands: any;
  let chatActions: ChatActions;
  let capturedChannel: MockChannel<StreamEvent> | null = null;

  beforeEach(() => {
    store = createStore();
    mockCommands = {
      sendMessage: vi.fn(),
    };
    
    const createChannel = (onMessage: (message: StreamEvent) => void) => {
      capturedChannel = createMockChannel(onMessage);
      return capturedChannel;
    };

    chatActions = new ChatActions(store, mockCommands, createChannel);
  });

  it('should add user message when sending message', async () => {
    const content = 'Hello, world!';
    
    // Mock successful command
    mockCommands.sendMessage.mockResolvedValueOnce(undefined);
    
    await chatActions.sendMessage(content);
    
    const state = store.getState();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].message.type).toBe('user');
    expect(state.messages[0].message.content).toBe(content);
  });

  it('should handle streaming response', async () => {
    const content = 'Test message';
    
    // Mock command that triggers streaming
    mockCommands.sendMessage.mockImplementation(async () => {
      if (capturedChannel) {
        await capturedChannel.simulateStream([
          { type: 'start', messageId: 'test-id' },
          { type: 'content', chunk: 'Hello' },
          { type: 'content', chunk: ' world!' },
          { type: 'done' },
        ] as StreamEvent[]);
      }
    });
    
    await chatActions.sendMessage(content);
    
    // Wait for async operations
    await new Promise(resolve => setTimeout(resolve, 100));
    
    const state = store.getState();
    expect(state.messages).toHaveLength(2); // User + Assistant
    expect(state.messages[1].message.content).toBe('Hello world!');
    expect(state.streamingMessage).toBeNull();
  });

  it('should handle tool calls during streaming', async () => {
    mockCommands.sendMessage.mockImplementation(async () => {
      if (capturedChannel) {
        await capturedChannel.simulateStream([
          { type: 'start', messageId: 'test-id' },
          { type: 'toolCallStart', id: 'tool-1', name: 'test_tool' },
          { type: 'toolCallComplete', id: 'tool-1' },
          { type: 'done' },
        ] as StreamEvent[]);
      }
    });

    await chatActions.sendMessage('Test');
    await new Promise(resolve => setTimeout(resolve, 100));

    const state = store.getState();
    expect(state.toolCalls.get('tool-1')).toBe('completed');
  });

  it('should clear conversation', () => {
    // Add some messages first
    store.setState(state => ({
      ...state,
      messages: [
        {
          id: '1',
          message: { type: 'user', content: 'Test', timestamp: new Date() },
        },
      ],
    }));

    chatActions.clearConversation();

    const state = store.getState();
    expect(state.messages).toHaveLength(0);
    expect(state.streamingMessage).toBeNull();
    expect(state.toolCalls.size).toBe(0);
  });
});
```

## Acceptance Criteria
- [ ] ChatActions handles all chat-related operations
- [ ] ConfigActions manages provider and MCP server configuration
- [ ] UIActions controls UI state
- [ ] All actions use channel factory injection for testability
- [ ] Actions properly handle async operations and errors
- [ ] State mutations only occur through actions
- [ ] Comprehensive test coverage for all action methods
- [ ] Actions are properly typed with TypeScript

## Dependencies
- Task 1: Setup Foundation
- Task 2: TypeScript Types

## Next Steps
After completing this task, proceed to:
- Task 4: Build block-based UI components
- Task 5: Add streaming support with Tauri Channels
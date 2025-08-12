# Task 6: Create Testing Infrastructure with Mock Channels

## Overview
Build comprehensive testing infrastructure with mock implementations for channels, commands, and state management. This ensures all components can be tested in isolation and integration scenarios can be simulated reliably.

## Goals
- Create robust mock channel implementations
- Set up comprehensive test utilities
- Implement integration testing patterns
- Add visual testing with component snapshots
- Ensure high test coverage across all layers
- Enable easy mocking of complex streaming scenarios

## Steps

### 6.1 Enhanced Mock Channel Implementation
**File**: `packages/aether-desktop/src/tests/mocks/mockChannel.ts` (update)

```typescript
import { Channel } from "@/types";

export interface MockChannelOptions {
  autoRespond?: boolean;
  responseDelay?: number;
  failureRate?: number; // 0-1, probability of failures
}

export class MockChannel<T> implements Channel<T> {
  id = crypto.randomUUID();
  onmessage?: (message: T) => void;
  
  private messages: T[] = [];
  private closed = false;
  private options: MockChannelOptions;

  constructor(
    onMessage?: (message: T) => void,
    options: MockChannelOptions = {}
  ) {
    this.onmessage = onMessage;
    this.options = {
      autoRespond: true,
      responseDelay: 0,
      failureRate: 0,
      ...options,
    };
  }

  async send(message: T): Promise<void> {
    if (this.closed) {
      throw new Error('Channel is closed');
    }

    this.messages.push(message);

    if (this.options.autoRespond && this.onmessage) {
      // Simulate network delay
      if (this.options.responseDelay > 0) {
        await new Promise(resolve => 
          setTimeout(resolve, this.options.responseDelay)
        );
      }

      // Simulate random failures
      if (Math.random() < this.options.failureRate) {
        throw new Error('Simulated channel failure');
      }

      this.onmessage(message);
    }
  }

  // Test helper methods
  async simulateStream(
    messages: T[], 
    delayMs = 10,
    options: { 
      randomDelay?: boolean;
      failAt?: number; // Fail at specific message index
    } = {}
  ): Promise<void> {
    for (let i = 0; i < messages.length; i++) {
      if (options.failAt === i) {
        throw new Error(`Simulated failure at message ${i}`);
      }

      await this.send(messages[i]);
      
      if (delayMs > 0) {
        const delay = options.randomDelay 
          ? delayMs + Math.random() * delayMs 
          : delayMs;
        await new Promise(resolve => setTimeout(resolve, delay));
      }
    }
  }

  async simulateStreamingContent(
    content: string,
    chunkSize = 5,
    delayMs = 50,
    messageIdPrefix = 'test'
  ): Promise<void> {
    // Simulate realistic streaming of content
    const chunks = this.chunkString(content, chunkSize);
    const messageId = `${messageIdPrefix}-${Date.now()}`;

    // Start event
    await this.send({
      type: 'start',
      messageId,
    } as any);

    // Content chunks
    for (const chunk of chunks) {
      await this.send({
        type: 'content',
        chunk,
      } as any);
      
      if (delayMs > 0) {
        await new Promise(resolve => setTimeout(resolve, delayMs));
      }
    }

    // Done event
    await this.send({ type: 'done' } as any);
  }

  simulateToolCall(
    toolId: string,
    toolName: string,
    params: any,
    result: string,
    delayMs = 100
  ): Promise<void> {
    return this.simulateStream([
      { type: 'toolCallStart', id: toolId, name: toolName },
      { type: 'toolCallArgument', id: toolId, chunk: JSON.stringify(params) },
      { type: 'toolCallComplete', id: toolId },
    ] as any, delayMs);
  }

  // Inspection methods for testing
  getMessages(): T[] {
    return [...this.messages];
  }

  getLastMessage(): T | undefined {
    return this.messages[this.messages.length - 1];
  }

  getMessageCount(): number {
    return this.messages.length;
  }

  clear(): void {
    this.messages = [];
  }

  close(): void {
    this.closed = true;
  }

  isClosed(): boolean {
    return this.closed;
  }

  // Wait for specific conditions
  async waitForMessage(
    predicate: (message: T) => boolean,
    timeoutMs = 5000
  ): Promise<T> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error(`Timeout waiting for message after ${timeoutMs}ms`));
      }, timeoutMs);

      const checkMessages = () => {
        const message = this.messages.find(predicate);
        if (message) {
          clearTimeout(timeout);
          resolve(message);
        } else {
          setTimeout(checkMessages, 10);
        }
      };

      checkMessages();
    });
  }

  async waitForMessageCount(count: number, timeoutMs = 5000): Promise<void> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error(
          `Timeout waiting for ${count} messages, got ${this.messages.length}`
        ));
      }, timeoutMs);

      const checkCount = () => {
        if (this.messages.length >= count) {
          clearTimeout(timeout);
          resolve();
        } else {
          setTimeout(checkCount, 10);
        }
      };

      checkCount();
    });
  }

  private chunkString(str: string, size: number): string[] {
    const chunks: string[] = [];
    for (let i = 0; i < str.length; i += size) {
      chunks.push(str.slice(i, i + size));
    }
    return chunks;
  }
}

export const createMockChannel = <T>(
  onMessage?: (message: T) => void,
  options?: MockChannelOptions
): MockChannel<T> => {
  return new MockChannel<T>(onMessage, options);
};

// Factory for creating multiple channels
export class MockChannelFactory {
  private channels: MockChannel<any>[] = [];

  createChannel<T>(
    onMessage?: (message: T) => void,
    options?: MockChannelOptions
  ): MockChannel<T> {
    const channel = new MockChannel<T>(onMessage, options);
    this.channels.push(channel);
    return channel;
  }

  getAllChannels(): MockChannel<any>[] {
    return [...this.channels];
  }

  closeAllChannels(): void {
    this.channels.forEach(channel => channel.close());
  }

  clearAllChannels(): void {
    this.channels.forEach(channel => channel.clear());
    this.channels = [];
  }
}
```

### 6.2 Create Mock Commands
**File**: `packages/aether-desktop/src/tests/mocks/mockCommands.ts`

```typescript
import { vi } from 'vitest';
import { StreamEvent, ToolDiscoveryEvent } from '@/types';
import { MockChannel } from './mockChannel';

export interface MockCommandsOptions {
  simulateLatency?: boolean;
  latencyMs?: number;
  failureRate?: number;
}

export class MockCommands {
  private options: MockCommandsOptions;
  
  // Spy functions for testing
  public sendMessage = vi.fn();
  public executeToolCall = vi.fn();
  public initializeAgent = vi.fn();
  public testProviderConnection = vi.fn();
  public connectMcpServer = vi.fn();
  public disconnectMcpServer = vi.fn();
  public discoverTools = vi.fn();
  public discoverToolsFromServer = vi.fn();

  constructor(options: MockCommandsOptions = {}) {
    this.options = {
      simulateLatency: false,
      latencyMs: 100,
      failureRate: 0,
      ...options,
    };

    this.setupDefaultBehaviors();
  }

  private setupDefaultBehaviors(): void {
    // Default sendMessage behavior
    this.sendMessage.mockImplementation(async (content: string, channel: MockChannel<StreamEvent>) => {
      if (this.shouldFail()) {
        throw new Error('Simulated command failure');
      }

      await this.simulateLatency();

      // Simulate realistic response
      await channel.simulateStreamingContent(
        `Response to: "${content}"`,
        3, // chunk size
        30  // delay between chunks
      );
    });

    // Default executeToolCall behavior
    this.executeToolCall.mockImplementation(async (
      toolId: string,
      toolName: string,
      params: any,
      channel: MockChannel<StreamEvent>
    ) => {
      if (this.shouldFail()) {
        throw new Error('Tool execution failed');
      }

      await this.simulateLatency();

      await channel.simulateToolCall(
        toolId,
        toolName,
        params,
        `Tool ${toolName} executed successfully`
      );

      return `Result from ${toolName}`;
    });

    // Default initializeAgent behavior
    this.initializeAgent.mockImplementation(async (config: any) => {
      if (this.shouldFail()) {
        throw new Error('Agent initialization failed');
      }

      await this.simulateLatency();
      return;
    });

    // Default testProviderConnection behavior
    this.testProviderConnection.mockImplementation(async (config: any) => {
      if (this.shouldFail()) {
        return { connected: false, error: 'Connection failed' };
      }

      await this.simulateLatency();
      return { connected: true, error: null };
    });

    // Default tool discovery behavior
    this.discoverTools.mockImplementation(async (channel: MockChannel<ToolDiscoveryEvent>) => {
      if (this.shouldFail()) {
        throw new Error('Tool discovery failed');
      }

      await this.simulateLatency();

      // Simulate discovering multiple tools
      const tools = [
        { name: 'search', description: 'Search the web', parameters: {} },
        { name: 'file_read', description: 'Read a file', parameters: {} },
        { name: 'calculator', description: 'Perform calculations', parameters: {} },
      ];

      for (const tool of tools) {
        await channel.send({
          type: 'discovered',
          tool,
        });
        await new Promise(resolve => setTimeout(resolve, 50));
      }

      await channel.send({
        type: 'complete',
        count: tools.length,
      });
    });
  }

  // Helper methods
  private async simulateLatency(): Promise<void> {
    if (this.options.simulateLatency) {
      const delay = this.options.latencyMs || 100;
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  private shouldFail(): boolean {
    return Math.random() < (this.options.failureRate || 0);
  }

  // Test utilities
  public reset(): void {
    this.sendMessage.mockClear();
    this.executeToolCall.mockClear();
    this.initializeAgent.mockClear();
    this.testProviderConnection.mockClear();
    this.connectMcpServer.mockClear();
    this.disconnectMcpServer.mockClear();
    this.discoverTools.mockClear();
    this.discoverToolsFromServer.mockClear();
  }

  public setFailureRate(rate: number): void {
    this.options.failureRate = rate;
  }

  public setLatency(enabled: boolean, ms = 100): void {
    this.options.simulateLatency = enabled;
    this.options.latencyMs = ms;
  }
}

export const createMockCommands = (options?: MockCommandsOptions): MockCommands => {
  return new MockCommands(options);
};
```

### 6.3 Create Test Context Provider
**File**: `packages/aether-desktop/src/tests/utils/TestContext.tsx`

```tsx
import React from 'react';
import { AppContext } from '@/hooks/useAppContext';
import { createStore, AppState } from '@/state/store';
import { createAppActions } from '@/state/actions';
import { MockChannelFactory } from '../mocks/mockChannel';
import { MockCommands } from '../mocks/mockCommands';

interface TestContextOptions {
  initialState?: Partial<AppState>;
  commandOptions?: any;
  channelOptions?: any;
}

export function createTestContext(options: TestContextOptions = {}) {
  const store = createStore({
    ...store.getState(),
    ...options.initialState,
  });

  const mockCommands = new MockCommands(options.commandOptions);
  const channelFactory = new MockChannelFactory();

  const createChannel = <T,>(onMessage: (message: T) => void) => {
    return channelFactory.createChannel<T>(onMessage, options.channelOptions);
  };

  const actions = createAppActions(store, mockCommands as any, createChannel);

  const context: AppContext = {
    commands: mockCommands as any,
    createChannel,
    store,
    actions,
  };

  return {
    context,
    store,
    mockCommands,
    channelFactory,
    actions,
  };
}

interface TestProviderProps {
  children: React.ReactNode;
  options?: TestContextOptions;
}

export const TestProvider: React.FC<TestProviderProps> = ({ 
  children, 
  options = {} 
}) => {
  const { context } = createTestContext(options);

  return (
    <AppContext.Provider value={context}>
      {children}
    </AppContext.Provider>
  );
};

// Higher-order component for easy testing
export function withTestContext<P extends object>(
  Component: React.ComponentType<P>,
  options?: TestContextOptions
) {
  return function TestWrappedComponent(props: P) {
    return (
      <TestProvider options={options}>
        <Component {...props} />
      </TestProvider>
    );
  };
}
```

### 6.4 Create Comprehensive Action Tests
**File**: `packages/aether-desktop/src/state/actions/__tests__/chat.integration.test.ts`

```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createTestContext } from '../../../tests/utils/TestContext';
import { StreamEvent } from '@/types';

describe('ChatActions Integration Tests', () => {
  let testContext: ReturnType<typeof createTestContext>;

  beforeEach(() => {
    testContext = createTestContext();
  });

  afterEach(() => {
    testContext.channelFactory.closeAllChannels();
    testContext.mockCommands.reset();
  });

  it('should handle complete conversation flow', async () => {
    const { actions, store, mockCommands, channelFactory } = testContext;

    // Send initial message
    await actions.chat.sendMessage('Hello, can you help me?');

    // Verify user message was added
    let state = store.getState();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].message.type).toBe('user');
    expect(state.messages[0].message.content).toBe('Hello, can you help me?');

    // Verify streaming started
    expect(state.streamingMessage).toBeTruthy();
    expect(state.streamingMessage?.isStreaming).toBe(true);

    // Wait for streaming to complete
    await new Promise(resolve => setTimeout(resolve, 200));

    // Verify final state
    state = store.getState();
    expect(state.messages).toHaveLength(2); // User + Assistant
    expect(state.streamingMessage).toBeNull();
    expect(state.messages[1].message.type).toBe('assistant');
  });

  it('should handle tool call workflow', async () => {
    const { actions, store, mockCommands } = testContext;

    // Mock sendMessage to include tool call
    mockCommands.sendMessage.mockImplementation(async (content, channel) => {
      await channel.simulateStream([
        { type: 'start', messageId: 'test-msg' },
        { type: 'content', chunk: 'I need to search for information.' },
        { type: 'toolCallStart', id: 'tool-1', name: 'search' },
        { type: 'toolCallArgument', id: 'tool-1', chunk: '{"query": "test"}' },
        { type: 'toolCallComplete', id: 'tool-1' },
        { type: 'content', chunk: ' Based on the search results...' },
        { type: 'done' },
      ] as StreamEvent[]);
    });

    await actions.chat.sendMessage('Search for information about testing');

    // Wait for streaming to complete
    await new Promise(resolve => setTimeout(resolve, 300));

    const state = store.getState();
    
    // Should have user message, tool call message, and assistant message
    expect(state.messages.length).toBeGreaterThanOrEqual(3);
    
    // Verify tool call state
    expect(state.toolCalls.get('tool-1')).toBe('completed');
    
    // Find tool call message
    const toolCallMessage = state.messages.find(
      m => m.message.type === 'tool_call'
    );
    expect(toolCallMessage).toBeTruthy();
    expect(toolCallMessage?.message.name).toBe('search');
  });

  it('should handle multiple concurrent tool calls', async () => {
    const { actions, store, mockCommands } = testContext;

    // Execute multiple tool calls concurrently
    const promises = [
      actions.chat.executeToolCall('tool-1', 'search', { query: 'test1' }),
      actions.chat.executeToolCall('tool-2', 'calculator', { expr: '2+2' }),
      actions.chat.executeToolCall('tool-3', 'file_read', { path: 'test.txt' }),
    ];

    await Promise.all(promises);

    // Wait for all operations to complete
    await new Promise(resolve => setTimeout(resolve, 200));

    const state = store.getState();
    
    // All tools should be completed
    expect(state.toolCalls.get('tool-1')).toBe('completed');
    expect(state.toolCalls.get('tool-2')).toBe('completed');
    expect(state.toolCalls.get('tool-3')).toBe('completed');

    // Should have tool result messages
    const toolResults = state.messages.filter(
      m => m.message.type === 'tool_result'
    );
    expect(toolResults).toHaveLength(3);
  });

  it('should handle streaming errors gracefully', async () => {
    const { actions, store, mockCommands } = testContext;

    // Mock command to fail mid-stream
    mockCommands.sendMessage.mockImplementation(async (content, channel) => {
      await channel.simulateStream([
        { type: 'start', messageId: 'test-msg' },
        { type: 'content', chunk: 'Starting response...' },
        { type: 'error', message: 'Connection lost' },
      ] as StreamEvent[]);
    });

    await actions.chat.sendMessage('Test error handling');

    // Wait for error handling
    await new Promise(resolve => setTimeout(resolve, 100));

    const state = store.getState();
    
    // Should have user message and error message
    expect(state.messages.length).toBeGreaterThanOrEqual(2);
    
    const errorMessage = state.messages.find(m => m.message.type === 'error');
    expect(errorMessage).toBeTruthy();
    expect(errorMessage?.message.message).toBe('Connection lost');
    
    // Streaming should be stopped
    expect(state.streamingMessage).toBeNull();
  });

  it('should handle conversation persistence', async () => {
    const { actions, store } = testContext;

    // Send multiple messages
    await actions.chat.sendMessage('First message');
    await new Promise(resolve => setTimeout(resolve, 100));
    
    await actions.chat.sendMessage('Second message');
    await new Promise(resolve => setTimeout(resolve, 100));

    let state = store.getState();
    const messageCount = state.messages.length;
    expect(messageCount).toBeGreaterThan(2);

    // Clear conversation
    actions.chat.clearConversation();

    state = store.getState();
    expect(state.messages).toHaveLength(0);
    expect(state.streamingMessage).toBeNull();
    expect(state.toolCalls.size).toBe(0);
  });
});
```

### 6.5 Create Component Integration Tests
**File**: `packages/aether-desktop/src/components/chat/__tests__/ChatView.integration.test.tsx`

```tsx
import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, beforeEach } from 'vitest';
import { ChatView } from '../ChatView';
import { TestProvider } from '../../../tests/utils/TestContext';

describe('ChatView Integration', () => {
  const renderChatView = (options = {}) => {
    return render(
      <TestProvider options={options}>
        <ChatView />
      </TestProvider>
    );
  };

  it('should render empty chat initially', () => {
    renderChatView();
    
    // Should show input but no messages
    expect(screen.getByPlaceholderText(/type your message/i)).toBeInTheDocument();
    expect(screen.queryByText('You')).not.toBeInTheDocument();
    expect(screen.queryByText('Assistant')).not.toBeInTheDocument();
  });

  it('should send message and show streaming response', async () => {
    renderChatView();
    
    const input = screen.getByPlaceholderText(/type your message/i);
    const sendButton = screen.getByRole('button', { name: /send/i });

    // Type and send message
    fireEvent.change(input, { target: { value: 'Hello, world!' } });
    fireEvent.click(sendButton);

    // Should show user message immediately
    expect(screen.getByText('You')).toBeInTheDocument();
    expect(screen.getByText('Hello, world!')).toBeInTheDocument();

    // Should show streaming indicator
    await waitFor(() => {
      expect(screen.getByText('Assistant')).toBeInTheDocument();
    });

    // Wait for streaming to complete
    await waitFor(() => {
      expect(screen.queryByText(/response to/i)).toBeInTheDocument();
    }, { timeout: 1000 });

    // Input should be cleared
    expect(input).toHaveValue('');
  });

  it('should handle keyboard shortcuts', async () => {
    renderChatView();
    
    const input = screen.getByPlaceholderText(/type your message/i);

    // Type message
    fireEvent.change(input, { target: { value: 'Test message' } });

    // Press Enter to send
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: false });

    // Should send message
    await waitFor(() => {
      expect(screen.getByText('Test message')).toBeInTheDocument();
    });

    // Test Shift+Enter for new line
    fireEvent.change(input, { target: { value: 'Line 1' } });
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    
    // Should not send message, just add new line
    expect(input).toHaveValue('Line 1\n');
  });

  it('should show error messages', async () => {
    // Configure to fail commands
    renderChatView({
      commandOptions: { failureRate: 1 }
    });
    
    const input = screen.getByPlaceholderText(/type your message/i);
    const sendButton = screen.getByRole('button', { name: /send/i });

    fireEvent.change(input, { target: { value: 'This will fail' } });
    fireEvent.click(sendButton);

    // Should show error message
    await waitFor(() => {
      expect(screen.getByText(/error/i)).toBeInTheDocument();
    });
  });

  it('should handle message collapsing', async () => {
    renderChatView({
      initialState: {
        messages: [
          {
            id: '1',
            message: {
              type: 'tool_call',
              id: 'tool-1',
              name: 'search',
              params: { query: 'test' },
              timestamp: new Date(),
            },
            collapsed: false,
          },
        ],
      },
    });

    // Should show expanded tool call
    expect(screen.getByText('Tool: search')).toBeInTheDocument();
    expect(screen.getByText(/parameters/i)).toBeInTheDocument();

    // Click collapse button
    const collapseButton = screen.getByRole('button', { name: /chevron/i });
    fireEvent.click(collapseButton);

    // Should hide parameters
    await waitFor(() => {
      expect(screen.queryByText(/parameters/i)).not.toBeInTheDocument();
    });
  });
});
```

### 6.6 Create Visual Regression Tests
**File**: `packages/aether-desktop/src/components/chat/__tests__/MessageBlock.visual.test.tsx`

```tsx
import React from 'react';
import { render } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { MessageBlock } from '../MessageBlock';
import { ChatMessageBlock, StreamingMessageBlock } from '@/types';

// Mock data for visual testing
const mockMessages = {
  user: {
    id: '1',
    message: {
      type: 'user' as const,
      content: 'Hello, world! This is a user message with some content.',
      timestamp: new Date('2023-01-01T12:00:00Z'),
    },
  },
  assistant: {
    id: '2',
    message: {
      type: 'assistant' as const,
      content: 'Hello! This is an assistant response with **markdown** formatting and `code blocks`.',
      timestamp: new Date('2023-01-01T12:01:00Z'),
    },
  },
  toolCall: {
    id: '3',
    message: {
      type: 'tool_call' as const,
      id: 'tool-1',
      name: 'search_web',
      params: { query: 'test search', limit: 10 },
      timestamp: new Date('2023-01-01T12:02:00Z'),
    },
    collapsed: false,
  },
  toolResult: {
    id: '4',
    message: {
      type: 'tool_result' as const,
      toolCallId: 'tool-1',
      content: 'Search results: Found 5 relevant items...',
      timestamp: new Date('2023-01-01T12:03:00Z'),
      success: true,
    },
  },
  error: {
    id: '5',
    message: {
      type: 'error' as const,
      message: 'Connection timeout occurred',
      timestamp: new Date('2023-01-01T12:04:00Z'),
      source: 'agent' as const,
    },
  },
  streaming: {
    id: '6',
    message: {
      type: 'assistant' as const,
      content: '',
      timestamp: new Date('2023-01-01T12:05:00Z'),
    },
    isStreaming: true,
    partialContent: 'This is a streaming response in progress...',
  },
} as const;

describe('MessageBlock Visual Tests', () => {
  it('should render user message block correctly', () => {
    const { container } = render(
      <MessageBlock block={mockMessages.user} />
    );
    expect(container.firstChild).toMatchSnapshot();
  });

  it('should render assistant message block correctly', () => {
    const { container } = render(
      <MessageBlock block={mockMessages.assistant} />
    );
    expect(container.firstChild).toMatchSnapshot();
  });

  it('should render tool call block correctly', () => {
    const { container } = render(
      <MessageBlock block={mockMessages.toolCall} />
    );
    expect(container.firstChild).toMatchSnapshot();
  });

  it('should render collapsed tool call block', () => {
    const collapsedToolCall = {
      ...mockMessages.toolCall,
      collapsed: true,
    };
    
    const { container } = render(
      <MessageBlock block={collapsedToolCall} />
    );
    expect(container.firstChild).toMatchSnapshot();
  });

  it('should render tool result block correctly', () => {
    const { container } = render(
      <MessageBlock block={mockMessages.toolResult} />
    );
    expect(container.firstChild).toMatchSnapshot();
  });

  it('should render error message block correctly', () => {
    const { container } = render(
      <MessageBlock block={mockMessages.error} />
    );
    expect(container.firstChild).toMatchSnapshot();
  });

  it('should render streaming message block correctly', () => {
    const { container } = render(
      <MessageBlock block={mockMessages.streaming} />
    );
    expect(container.firstChild).toMatchSnapshot();
  });

  it('should render with dark theme', () => {
    // Add dark theme class to container
    const { container } = render(
      <div className="dark">
        <MessageBlock block={mockMessages.assistant} />
      </div>
    );
    expect(container.firstChild).toMatchSnapshot();
  });
});
```

### 6.7 Create Performance Tests
**File**: `packages/aether-desktop/src/tests/performance/rendering.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { MessageList } from '../../components/chat/MessageList';
import { TestProvider } from '../utils/TestContext';
import { ChatMessageBlock } from '@/types';

describe('Performance Tests', () => {
  const generateMockMessages = (count: number): ChatMessageBlock[] => {
    return Array.from({ length: count }, (_, i) => ({
      id: `msg-${i}`,
      message: {
        type: i % 3 === 0 ? 'user' : 'assistant',
        content: `Message ${i}: ${'Lorem ipsum '.repeat(20)}`,
        timestamp: new Date(Date.now() - (count - i) * 1000),
      },
    }));
  };

  it('should render large message list efficiently', () => {
    const messages = generateMockMessages(1000);
    
    const startTime = performance.now();
    
    render(
      <TestProvider options={{ initialState: { messages } }}>
        <MessageList />
      </TestProvider>
    );

    const endTime = performance.now();
    const renderTime = endTime - startTime;

    // Should render within reasonable time
    expect(renderTime).toBeLessThan(1000); // 1 second
    
    // Should show messages
    expect(screen.getByText(/Message 0:/)).toBeInTheDocument();
  });

  it('should handle rapid state updates efficiently', async () => {
    const { rerender } = render(
      <TestProvider>
        <MessageList />
      </TestProvider>
    );

    const iterations = 100;
    const startTime = performance.now();

    // Simulate rapid updates
    for (let i = 0; i < iterations; i++) {
      const messages = generateMockMessages(i + 1);
      
      rerender(
        <TestProvider options={{ initialState: { messages } }}>
          <MessageList />
        </TestProvider>
      );
    }

    const endTime = performance.now();
    const totalTime = endTime - startTime;
    const averageTime = totalTime / iterations;

    // Each update should be fast
    expect(averageTime).toBeLessThan(10); // 10ms per update
  });
});
```

### 6.8 Create E2E Test Setup
**File**: `packages/aether-desktop/tests-e2e/chat.spec.ts`

```typescript
import { test, expect } from '@playwright/test';

test.describe('Chat Interface', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('should send and receive messages', async ({ page }) => {
    // Type message
    await page.fill('[placeholder*="Type your message"]', 'Hello, assistant!');
    
    // Send message
    await page.click('button:has-text("Send")');
    
    // Verify user message appears
    await expect(page.locator('text=You')).toBeVisible();
    await expect(page.locator('text=Hello, assistant!')).toBeVisible();
    
    // Verify assistant response appears
    await expect(page.locator('text=Assistant')).toBeVisible({ timeout: 5000 });
    
    // Verify input is cleared
    await expect(page.locator('[placeholder*="Type your message"]')).toHaveValue('');
  });

  test('should handle keyboard shortcuts', async ({ page }) => {
    const input = page.locator('[placeholder*="Type your message"]');
    
    // Type message and press Enter
    await input.fill('Test keyboard shortcut');
    await input.press('Enter');
    
    // Should send message
    await expect(page.locator('text=Test keyboard shortcut')).toBeVisible();
    
    // Test Shift+Enter for new line
    await input.fill('Line 1');
    await input.press('Shift+Enter');
    await input.type('Line 2');
    
    // Should have multiline content
    await expect(input).toHaveValue('Line 1\nLine 2');
  });

  test('should show streaming indicator', async ({ page }) => {
    // Send message
    await page.fill('[placeholder*="Type your message"]', 'Tell me a story');
    await page.click('button:has-text("Send")');
    
    // Should show streaming indicator
    await expect(page.locator('[data-testid="streaming-indicator"]')).toBeVisible();
    
    // Should show stop button
    await expect(page.locator('button:has-text("Stop")')).toBeVisible();
  });

  test('should handle tool calls', async ({ page }) => {
    // Mock tool call scenario
    await page.route('**/api/send_message', async route => {
      // Simulate tool call response
      await route.fulfill({
        status: 200,
        body: JSON.stringify({
          events: [
            { type: 'toolCallStart', id: 'tool-1', name: 'search' },
            { type: 'toolCallComplete', id: 'tool-1' },
          ]
        })
      });
    });
    
    await page.fill('[placeholder*="Type your message"]', 'Search for information');
    await page.click('button:has-text("Send")');
    
    // Should show tool call block
    await expect(page.locator('text=Tool: search')).toBeVisible();
  });
});
```

### 6.9 Update Package.json Scripts
**File**: `packages/aether-desktop/package.json` (update scripts section)

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "test": "vitest",
    "test:ui": "vitest --ui",
    "test:coverage": "vitest --coverage",
    "test:watch": "vitest --watch",
    "test:integration": "vitest --run tests/integration",
    "test:e2e": "playwright test",
    "test:e2e:ui": "playwright test --ui",
    "test:visual": "vitest --run visual",
    "test:all": "npm run test && npm run test:e2e"
  }
}
```

## Acceptance Criteria
- [ ] Comprehensive mock implementations for all external dependencies
- [ ] Test utilities that enable easy component and integration testing
- [ ] High test coverage across all application layers
- [ ] Visual regression testing for UI components
- [ ] Performance tests for large data scenarios
- [ ] E2E tests covering critical user journeys
- [ ] Easy-to-use test patterns and utilities
- [ ] Fast test execution and reliable test results
- [ ] Mocks that accurately simulate real behavior
- [ ] Clear separation between unit, integration, and E2E tests

## Dependencies
- All previous tasks (foundational components needed for testing)

## Next Steps
After completing this task:
- Run comprehensive test suite to ensure quality
- Add continuous integration setup
- Implement remaining features with test-driven development
- Add advanced features like virtual scrolling and markdown rendering
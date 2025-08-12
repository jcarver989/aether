# Aether Desktop Frontend Implementation Plan

## Overview
Building a Tauri + React frontend for Aether following a clean architecture pattern with:
- Zustand for state management
- Action classes with dependency injection for testability
- React Context for providing dependencies
- Block-based message UI inspired by the TUI implementation
- Tauri Channels for real-time streaming
- shadcn/ui + Tailwind CSS for styling

## Phase 1: Core State & Types

### AppState Extension
```typescript
interface AppState {
  // Chat Domain
  messages: ChatMessageBlock[]
  streamingMessage: StreamingMessageBlock | null
  scrollOffset: number
  selectedMessageId: string | null
  toolCalls: Map<string, ToolCallState>
  
  // Configuration Domain
  activeProvider: 'openrouter' | 'ollama'
  providerConfigs: {
    openrouter: { apiKey: string; model: string }
    ollama: { baseUrl: string; model: string }
  }
  mcpServers: McpServerConfig[]
  availableTools: ToolDefinition[]
  
  // UI State
  theme: 'light' | 'dark'
  sidebarOpen: boolean
}
```

### Type Definitions
```typescript
// Core message types matching aether_core
interface ChatMessageBlock {
  id: string
  message: ChatMessage
  height?: number
  collapsed?: boolean  // for tool calls
}

interface StreamingMessageBlock extends ChatMessageBlock {
  isStreaming: boolean
  partialContent: string
}

type ChatMessage = 
  | { type: 'system'; content: string; timestamp: Date }
  | { type: 'user'; content: string; timestamp: Date }
  | { type: 'assistant'; content: string; timestamp: Date }
  | { type: 'tool_call'; id: string; name: string; params: any; timestamp: Date }
  | { type: 'tool_result'; toolCallId: string; content: string; timestamp: Date }
  | { type: 'error'; message: string; timestamp: Date }

// Event types for streaming
type StreamEvent = 
  | { type: 'start'; messageId: string }
  | { type: 'content'; chunk: string }
  | { type: 'toolCallStart'; id: string; name: string }
  | { type: 'toolCallArgument'; id: string; chunk: string }
  | { type: 'toolCallComplete'; id: string }
  | { type: 'done' }
  | { type: 'error'; message: string }

type ToolDiscoveryEvent = 
  | { type: 'discovered'; tool: ToolDefinition }
  | { type: 'complete'; count: number }
  | { type: 'error'; message: string }
```

## Phase 2: Tauri Backend Integration

### Rust Dependencies
Add to `src-tauri/Cargo.toml`:
```toml
[dependencies]
aether_core = { path = "../aether_core" }
uuid = { version = "1.0", features = ["v4"] }
tokio-stream = "0.1"
```

### Channel-based Commands
```rust
// src-tauri/src/commands/chat.rs
#[derive(Clone, serde::Serialize)]
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
pub async fn send_message(
    content: String,
    on_stream: Channel<StreamEvent>,
    state: State<'_, AgentState>,
) -> Result<(), String>

#[tauri::command]
pub async fn execute_tool(
    tool_id: String,
    name: String,
    params: serde_json::Value,
    on_stream: Channel<StreamEvent>,
    state: State<'_, AgentState>,
) -> Result<String, String>
```

### State Management
```rust
// src-tauri/src/state.rs
pub struct AgentState {
    pub agent: Arc<Mutex<Agent<Box<dyn LlmProvider>>>>,
    pub tool_registry: Arc<Mutex<ToolRegistry>>,
}
```

## Phase 3: Action Classes with Channel Factory Injection

### ChatActions Implementation
```typescript
// state/actions/chat.ts
export class ChatActions {
  constructor(
    private store: ZustandStore<AppState>,
    private tauriCommands: typeof commands,
    private createChannel: <T>(onMessage: (message: T) => void) => Channel<T>
  ) {}

  async sendMessage(content: string) {
    // 1. Add user message block
    // 2. Create streaming assistant message
    // 3. Set up channel handler for streaming events
    // 4. Invoke Tauri command with channel
    // 5. Handle stream completion/errors
  }

  async executeToolCall(toolId: string, name: string, params: any) {
    // Similar pattern for tool execution
  }

  private handleStreamEvent(event: StreamEvent, messageId: string) {
    // Route events to appropriate handlers
  }
}
```

### ConfigActions Implementation
```typescript
// state/actions/config.ts
export class ConfigActions {
  constructor(
    private store: ZustandStore<AppState>,
    private tauriCommands: typeof commands,
    private createChannel: <T>(onMessage: (message: T) => void) => Channel<T>
  ) {}

  async connectMcpServer(config: McpServerConfig) {
    // Connect to MCP server and update state
  }

  async discoverTools() {
    // Stream tool discovery events
  }

  async selectProvider(provider: 'openrouter' | 'ollama') {
    // Switch LLM provider
  }
}
```

## Phase 4: Block-Based UI Components

### Component Structure
```
components/
├── chat/
│   ├── ChatView.tsx          // Main container with scroll
│   ├── MessageList.tsx       // Virtualized list of blocks
│   ├── MessageBlock.tsx      // Router for message types
│   ├── StreamingIndicator.tsx
│   ├── blocks/
│   │   ├── SystemBlock.tsx   
│   │   ├── UserBlock.tsx     
│   │   ├── AssistantBlock.tsx
│   │   ├── StreamingAssistantBlock.tsx
│   │   ├── ToolCallBlock.tsx  // Collapsible
│   │   └── ToolResultBlock.tsx
│   └── BlockHeader.tsx       // Reusable header with title/controls
├── input/
│   ├── ChatInput.tsx         // Multi-line with shortcuts
│   └── CommandPalette.tsx    // Slash commands
├── settings/
│   ├── ProviderConfig.tsx
│   ├── McpServerList.tsx
│   └── ApiKeyForm.tsx
└── ui/                       // shadcn/ui components
```

### Block Design System
Each block follows consistent styling:
- Rounded borders (`rounded-lg`)
- Role-based color coding
- Consistent padding (`p-4`)
- Hover effects for interactive elements
- Collapse/expand animations
- Loading states with subtle animations

### Theme Colors
```typescript
const blockTheme = {
  system: 'border-gray-400 bg-gray-50',
  user: 'border-blue-400 bg-blue-50',
  assistant: 'border-green-400 bg-green-50',
  toolCall: 'border-purple-400 bg-purple-50',
  toolResult: 'border-orange-400 bg-orange-50',
  error: 'border-red-400 bg-red-50',
  streaming: 'animate-pulse-subtle'
}
```

## Phase 5: AppContext Setup

### Real Implementation
```typescript
// App.tsx
import { Channel } from '@tauri-apps/api/core';

const createRealChannel = <T,>(onMessage: (message: T) => void): Channel<T> => {
  const channel = new Channel<T>();
  channel.onmessage = onMessage;
  return channel;
};

const store = createStore();
const chatActions = new ChatActions(store, commands, createRealChannel);
const configActions = new ConfigActions(store, commands, createRealChannel);

const appContext: AppContext = {
  commands,
  createChannel: createRealChannel,
  store,
  actions: { chat: chatActions, config: configActions }
};
```

### Context Interface
```typescript
export interface AppContext {
  commands: typeof commands;
  createChannel: <T>(onMessage: (message: T) => void) => Channel<T>;
  store: ZustandStore<AppState>;
  actions: {
    chat: ChatActions;
    config: ConfigActions;
  };
}
```

## Phase 6: Testing Infrastructure

### Mock Channel Implementation
```typescript
// tests/mocks/mockChannel.ts
export class MockChannel<T> implements Channel<T> {
  id = crypto.randomUUID();
  onmessage?: (message: T) => void;
  private messages: T[] = [];
  
  send(message: T) {
    this.messages.push(message);
    setTimeout(() => this.onmessage?.(message), 0);
  }
  
  async simulateStream(messages: T[], delayMs = 10) {
    for (const message of messages) {
      this.send(message);
      await new Promise(resolve => setTimeout(resolve, delayMs));
    }
  }
}
```

### Test Patterns
```typescript
// Tests follow pattern:
// 1. Create mock store and commands
// 2. Inject mock channel factory
// 3. Create action instance
// 4. Test behavior
// 5. Assert state changes

describe('ChatActions', () => {
  it('should handle streaming messages', async () => {
    const store = createStore();
    const mockCommands = { sendMessage: vi.fn() };
    const actions = new ChatActions(store, mockCommands, createMockChannel);
    
    await actions.sendMessage('Test');
    // Assert behavior
  });
});
```

## Phase 7: Advanced Features

### Virtual Scrolling
- Use `@tanstack/react-virtual` for performance
- Cache calculated block heights
- Auto-scroll to bottom on new messages
- Smooth scroll to specific messages

### Markdown Rendering
- `react-markdown` with plugins
- Syntax highlighting with `prism-react-renderer`
- Copy code blocks on hover
- LaTeX math support (optional)

### Keyboard Shortcuts
- Enter: Send message
- Shift+Enter: New line
- Ctrl+/: Command palette
- Escape: Close modals/cancel streaming
- Arrow keys: Navigate messages

### Progressive Enhancement
- Show partial tool results during execution
- Graceful degradation when streaming fails
- Offline mode indicators
- Retry mechanisms for failed operations

## Implementation Order

1. **Setup Foundation** (Phase 2-3)
   - Add aether_core to Tauri backend
   - Generate TypeScript types with tauri-specta
   - Implement basic Action classes
   - Set up AppContext with real channels

2. **Core UI** (Phase 4)
   - Build basic message blocks
   - Implement ChatView with static messages
   - Add input component
   - Style with shadcn/ui components

3. **Streaming Support** (Phase 5)
   - Implement streaming message handling
   - Add real-time UI updates
   - Handle tool call streaming
   - Add error handling

4. **Testing** (Phase 6)
   - Create mock implementations
   - Write unit tests for Actions
   - Integration tests for UI components
   - E2E tests with Playwright

5. **Polish** (Phase 7)
   - Add virtual scrolling
   - Implement advanced features
   - Performance optimization
   - Accessibility improvements

## File Structure

```
packages/aether-desktop/
├── src/
│   ├── components/
│   │   ├── chat/
│   │   ├── input/
│   │   ├── settings/
│   │   └── ui/              // shadcn/ui
│   ├── state/
│   │   ├── store.ts
│   │   ├── actions/
│   │   │   ├── chat.ts
│   │   │   └── config.ts
│   │   ├── selectors.ts
│   │   └── index.ts
│   ├── hooks/
│   │   ├── useAppContext.ts
│   │   ├── useSelector.ts
│   │   └── useStreaming.ts
│   ├── types/
│   │   ├── chat.ts
│   │   ├── events.ts
│   │   └── config.ts
│   ├── lib/
│   │   ├── utils.ts
│   │   ├── markdown.ts
│   │   └── theme.ts
│   ├── generated/
│   │   └── invoke.ts        // tauri-specta output
│   └── tests/
│       ├── mocks/
│       ├── actions/
│       └── components/
├── src-tauri/
│   └── src/
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── chat.rs
│       │   └── config.rs
│       ├── state.rs
│       └── lib.rs
└── tests-e2e/              // Playwright tests
```

## Success Metrics

- [ ] Real-time streaming messages display correctly
- [ ] Tool calls execute and show progress
- [ ] Block-based UI matches TUI design language
- [ ] All components are fully tested
- [ ] Type-safe communication between Rust and TypeScript
- [ ] Smooth performance with large conversation histories
- [ ] Accessible keyboard navigation
- [ ] Error states handled gracefully
# Task 1: Setup Foundation

## Overview
Set up the foundational infrastructure for the Aether desktop frontend, including Tauri backend integration with aether_core and basic TypeScript type generation.

## Goals
- Integrate aether_core into Tauri backend
- Set up tauri-specta for type generation
- Create basic state management structure
- Establish channel communication patterns

## Steps

### 1.1 Add aether_core to Tauri Dependencies
**File**: `packages/aether-desktop/src-tauri/Cargo.toml`

Add dependencies:
```toml
[dependencies]
aether_core = { path = "../../aether_core" }
uuid = { version = "1.0", features = ["v4"] }
tokio-stream = "0.1"
futures = "0.3"
```

### 1.2 Create Agent State Management
**File**: `packages/aether-desktop/src-tauri/src/state.rs`

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use aether_core::{
    agent::Agent,
    llm::LlmProvider,
    tools::ToolRegistry,
};

pub struct AgentState {
    pub agent: Arc<Mutex<Option<Agent<Box<dyn LlmProvider>>>>>,
    pub tool_registry: Arc<Mutex<ToolRegistry>>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            agent: Arc::new(Mutex::new(None)),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
        }
    }
}
```

### 1.3 Create Basic Command Structure
**File**: `packages/aether-desktop/src-tauri/src/commands/mod.rs`

```rust
pub mod chat;
pub mod config;

pub use chat::*;
pub use config::*;
```

### 1.4 Set up Basic Event Types
**File**: `packages/aether-desktop/src-tauri/src/commands/chat.rs`

```rust
use tauri::ipc::Channel;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
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
pub async fn example_command() -> Result<String, String> {
    Ok("Hello from Rust!".to_string())
}
```

### 1.5 Update Tauri Main
**File**: `packages/aether-desktop/src-tauri/src/lib.rs`

```rust
mod state;
mod commands;

use state::AgentState;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AgentState::default())
        .invoke_handler(tauri::generate_handler![
            example_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 1.6 Generate TypeScript Types
**File**: `packages/aether-desktop/src-tauri/src/commands/chat.rs` (add to existing)

```rust
use specta::Type;
use tauri_specta::Event;

#[derive(Serialize, Type, Event)]
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
```

### 1.7 Create Generated Types Build Script
**File**: `packages/aether-desktop/src-tauri/src/commands/mod.rs` (update)

```rust
use tauri_specta::{collect_commands, collect_events, Builder};

pub mod chat;
pub mod config;

pub use chat::*;
pub use config::*;

#[cfg(debug_assertions)]
pub fn generate_types() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![example_command])
        .events(collect_events![StreamEvent]);

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/generated/invoke.ts",
        )
        .expect("Failed to export typescript types");
}
```

### 1.8 Basic State Store Setup
**File**: `packages/aether-desktop/src/state/store.ts` (update existing)

```typescript
import { create, StoreApi, UseBoundStore } from "zustand";

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
  | { type: 'system'; content: string; timestamp: Date }
  | { type: 'user'; content: string; timestamp: Date }
  | { type: 'assistant'; content: string; timestamp: Date }
  | { type: 'tool_call'; id: string; name: string; params: any; timestamp: Date }
  | { type: 'tool_result'; toolCallId: string; content: string; timestamp: Date }
  | { type: 'error'; message: string; timestamp: Date };

export interface AppState {
  // Chat Domain
  messages: ChatMessageBlock[];
  streamingMessage: StreamingMessageBlock | null;
  scrollOffset: number;
  selectedMessageId: string | null;
  
  // Simple message for now
  message: string;
}

export type ZustandStore<T> = UseBoundStore<StoreApi<T>>;

export function createStore(initialState: AppState = defaultAppState()): ZustandStore<AppState> {
  return create<AppState>(() => initialState);
}

export function defaultAppState(): AppState {
  return {
    messages: [],
    streamingMessage: null,
    scrollOffset: 0,
    selectedMessageId: null,
    message: "Hello, world!",
  };
}
```

### 1.9 Create Basic Action Structure
**File**: `packages/aether-desktop/src/state/actions.ts` (update existing)

```typescript
import { commands } from "@/generated/invoke";
import { AppState, ZustandStore } from "./store";
import { Channel } from "@tauri-apps/api/core";

export class AppActions {
  constructor(
    private store: ZustandStore<AppState>,
    private tauriCommands: typeof commands,
    private createChannel: <T>(onMessage: (message: T) => void) => Channel<T>,
  ) {}

  async exampleAction() {
    await this.tauriCommands.exampleCommand();

    this.store.setState((state) => ({
      ...state,
      message: "Action executed!",
    }));
  }
}
```

## Testing

### 1.10 Test Basic Setup
1. Run `cargo build` in `src-tauri/` to verify Rust compilation
2. Run `npm run dev` to start development server
3. Verify TypeScript types are generated in `src/generated/invoke.ts`
4. Test that basic Tauri command invocation works

## Acceptance Criteria
- [ ] aether_core successfully integrated into Tauri backend
- [ ] Basic state management structure in place
- [ ] TypeScript types generated from Rust
- [ ] Example command can be invoked from frontend
- [ ] No compilation errors in Rust or TypeScript
- [ ] Development server starts successfully

## Dependencies
- None (this is the foundation task)

## Next Steps
After completing this task, proceed to:
- Task 2: Create TypeScript types for chat messages and events
- Task 3: Implement Action classes with channel factory injection
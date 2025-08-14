import { create, StoreApi, UseBoundStore } from "zustand";

// Import generated types from Rust
import type { 
  ToolCallState,
  AppConfig,
  AppStatus
} from "../generated/bindings";

// Import UI types
import type { 
  ChatMessageBlock, 
  StreamingMessageBlock, 
  UIState, 
  ScrollState 
} from "../types/ui";

export interface AppState {
  // Chat Domain
  messages: ChatMessageBlock[];
  streamingMessage: StreamingMessageBlock | null;
  toolCalls: Map<string, ToolCallState>;
  
  // Configuration Domain
  config: AppConfig | null;
  status: AppStatus | null;
  
  // UI State
  ui: UIState;
  scroll: ScrollState;
  selectedMessageId: string | null;
}

export type ZustandStore<T> = UseBoundStore<StoreApi<T>>;

export function createStore(): ZustandStore<AppState> {
  return create<AppState>(() => ({
    ...defaultAppState(),
  }));
}

export function defaultAppState(): AppState {
  return {
    // Chat Domain
    messages: [],
    streamingMessage: null,
    toolCalls: new Map(),
    
    // Configuration Domain
    config: null,
    status: null,
    
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



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
  
  // Domain methods for state mutations
  setMessages: (messages: ChatMessageBlock[]) => void;
  addMessage: (message: ChatMessageBlock) => void;
  setStreamingMessage: (message: StreamingMessageBlock | null) => void;
  updateToolCallState: (id: string, state: ToolCallState) => void;
  setConfig: (config: AppConfig) => void;
  setStatus: (status: AppStatus) => void;
  updateUI: (ui: Partial<UIState>) => void;
  updateScroll: (scroll: Partial<ScrollState>) => void;
  setSelectedMessage: (id: string | null) => void;
}

export type ZustandStore<T> = UseBoundStore<StoreApi<T>>;

export function createStore(): ZustandStore<AppState> {
  return create<AppState>((set) => ({
    ...defaultAppState(),
    
    // Actions
    setMessages: (messages) => set({ messages }),
    addMessage: (message) => set((state) => ({ 
      messages: [...state.messages, message] 
    })),
    setStreamingMessage: (streamingMessage) => set({ streamingMessage }),
    updateToolCallState: (id, state) => set((prev) => {
      const newToolCalls = new Map(prev.toolCalls);
      newToolCalls.set(id, state);
      return { toolCalls: newToolCalls };
    }),
    setConfig: (config) => set({ config }),
    setStatus: (status) => set({ status }),
    updateUI: (ui) => set((state) => ({ 
      ui: { ...state.ui, ...ui } 
    })),
    updateScroll: (scroll) => set((state) => ({ 
      scroll: { ...state.scroll, ...scroll } 
    })),
    setSelectedMessage: (selectedMessageId) => set({ selectedMessageId }),
  }));
}

export function defaultAppState(): Omit<AppState, 'setMessages' | 'addMessage' | 'setStreamingMessage' | 'updateToolCallState' | 'setConfig' | 'setStatus' | 'updateUI' | 'updateScroll' | 'setSelectedMessage'> {
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



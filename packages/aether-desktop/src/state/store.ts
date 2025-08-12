import { create, StoreApi, UseBoundStore } from "zustand";

// Import generated types from Rust
import type { 
  ChatMessage, 
  StreamChunk, 
  ToolCall, 
  ToolCallState,
  StreamEvent
} from "../generated/bindings";

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

export interface AppState {
  // Chat Domain
  messages: ChatMessageBlock[];
  streamingMessage: StreamingMessageBlock | null;
  scrollOffset: number;
  selectedMessageId: string | null;
  
  // Simple message for backward compatibility
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



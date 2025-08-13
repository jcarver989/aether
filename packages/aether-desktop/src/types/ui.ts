// UI-specific types that don't need Rust equivalents
// Import the generated ChatMessage type
import type { ChatMessage } from "../generated/bindings";

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

// UI-specific message block types
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

// Utility types for UI components
export type AsyncResult<T> = Promise<T | Error>;
export type Optional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;
export type RequiredFields<T, K extends keyof T> = T & Required<Pick<T, K>>;

// Import Tauri's Channel type
import { Channel as TauriChannel } from "@tauri-apps/api/core";

// Factory type for creating channels - use Tauri's Channel directly
export type ChannelFactory = <T>(onMessage: (message: T) => void) => TauriChannel<T>;

// Real channel factory that creates Tauri channels
export function createTauriChannelFactory(): ChannelFactory {
  return function createChannel<T>(onMessage: (message: T) => void): TauriChannel<T> {
    // Create a Tauri channel using the Channel API
    const tauriChannel = new TauriChannel(onMessage);
    
    // Return the Tauri channel directly
    return tauriChannel;
  };
}
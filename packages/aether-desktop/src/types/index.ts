// Central type exports - single source of truth for all types

// Re-export all generated types from Rust/Tauri
export type {
  ChatMessage,
  ToolCall,
  ToolCallState,
  StreamEvent,
  ToolDiscoveryEvent,
  ToolDefinition,
  LlmProvider,
  OpenRouterConfig,
  OllamaConfig,
  ConnectionStatus,
  ProviderStatus,
  McpServerStatus,
  McpServerConfig,
  AppConfig,
  McpServerWithId,
  AppStatus,
  StreamChunk,
  ChatStreamEvent,
  ToolDiscoveryEventWrapper,
  SendMessageRequest,
  SendMessageResponse,
} from "../generated/bindings";

// Re-export UI-specific types
export type {
  Theme,
  UIState,
  ScrollState,
  BlockTheme,
  ChatMessageBlock,
  StreamingMessageBlock,
  AsyncResult,
  Optional,
  RequiredFields,
  ChannelFactory,
} from "./ui";

// Re-export type guards and utilities
export * from "./guards";

// Re-export store types
export type { AppState, ZustandStore } from "../state/store";

// Import types for utility combinations
import type { ChatMessageBlock } from "./ui";
import type { 
  ChatMessage, 
  LlmProvider, 
  OpenRouterConfig, 
  OllamaConfig, 
  McpServerWithId, 
  McpServerStatus,
  AppConfig,
  StreamEvent,
  ToolDiscoveryEvent
} from "../generated/bindings";
import type { StreamingMessageBlock } from "./ui";

// Utility type combinations for common use cases
export type MessageWithBlock = {
  block: ChatMessageBlock;
  isStreaming: boolean;
};

export type ConfiguredProvider = {
  provider: LlmProvider;
  config: OpenRouterConfig | OllamaConfig;
};

export type ServerWithStatus = {
  server: McpServerWithId;
  status: McpServerStatus;
};

// Event handler types for components
export type MessageEventHandler = (message: ChatMessage) => void;
export type StreamEventHandler = (event: StreamEvent) => void;
export type ToolDiscoveryEventHandler = (event: ToolDiscoveryEvent) => void;
export type ConfigUpdateHandler = (config: AppConfig) => void;

// Component prop types for common patterns
export interface BaseComponentProps {
  className?: string;
  children?: React.ReactNode;
}

export interface MessageComponentProps extends BaseComponentProps {
  message: ChatMessage;
  onEdit?: MessageEventHandler;
  onDelete?: (messageId: string) => void;
  onCopy?: (content: string) => void;
}

export interface StreamingComponentProps extends BaseComponentProps {
  streamingMessage: StreamingMessageBlock;
  onCancel?: () => void;
}

export interface ConfigComponentProps extends BaseComponentProps {
  config: AppConfig;
  onUpdate: ConfigUpdateHandler;
  onReset?: () => void;
}

// Form types for configuration UI
export interface ProviderFormData {
  openrouter: {
    apiKey: string;
    model: string;
    baseUrl: string;
    temperature: number;
  };
  ollama: {
    baseUrl: string;
    model: string;
    temperature: number;
  };
}

export interface McpServerFormData {
  name: string;
  enabled: boolean;
  type: 'http' | 'stdio';
  httpConfig?: {
    url: string;
    headers: Record<string, string>;
  };
  stdioConfig?: {
    command: string;
    args: string[];
    env: Record<string, string>;
  };
}

// Error types for better error handling
export interface AppError {
  code: string;
  message: string;
  details?: any;
  timestamp: Date;
}

export type ErrorSeverity = 'info' | 'warning' | 'error' | 'critical';

export interface NotificationEvent {
  id: string;
  type: ErrorSeverity;
  title: string;
  message: string;
  actions?: Array<{
    label: string;
    action: () => void;
  }>;
  autoClose?: boolean;
  duration?: number;
}
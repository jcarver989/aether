import type {
  ChatMessage,
  SendMessageRequest,
  LlmProvider,
  AppConfig,
  InitializeAgentRequest,
  ChatStreamEvent,
  OpenRouterConfig,
  OllamaConfig
} from "../generated/bindings";
import { commands } from "../generated/bindings";
import type { AppState, ZustandStore } from "./store";
import type {
  ChatMessageBlock,
  StreamingMessageBlock,
  ChannelFactory,
  Theme
} from "../types/ui";

export class AppActions {
  private streamingBuffer = '';
  private streamingUpdateRAF: number | null = null;
  private pendingUpdate = false;

  constructor(
    private store: ZustandStore<AppState>,
    private createChannel: ChannelFactory,
  ) { }

  async sendMessage(content: string): Promise<void> {
    const userMessageId = crypto.randomUUID();
    const assistantMessageId = crypto.randomUUID();

    try {
      // Add user message to store immediately
      this.addUserMessage(userMessageId, content);

      // Create streaming assistant message
      this.startStreamingMessage(assistantMessageId);

      // Set up streaming channel
      const channel = this.createChannel<ChatStreamEvent>((event) => {
        this.handleStreamEvent(event, assistantMessageId);
      });

      // Send message request with channel for streaming
      const request: SendMessageRequest = {
        content,
        message_id: userMessageId,
      };

      const result = await commands.sendMessage(request, channel);
      if (result.status === "error") {
        throw new Error(result.error);
      }

    } catch (error) {
      this.handleStreamError(error as Error, assistantMessageId);
    }
  }

  private addUserMessage(id: string, content: string): void {
    const userMessage: ChatMessage = {
      type: "user",
      content,
      timestamp: new Date().toISOString(),
    };

    const userBlock: ChatMessageBlock = {
      id,
      message: userMessage,
    };

    this.store.setState(state => ({
      ...state,
      messages: [...state.messages, userBlock]
    }));
  }

  private startStreamingMessage(id: string): void {
    this.streamingBuffer = '';

    const streamingMessage: StreamingMessageBlock = {
      id,
      message: {
        type: "assistantStreaming",
        content: "",
        timestamp: new Date().toISOString(),
      },
      isStreaming: true,
      partialContent: "",
    };

    this.store.setState(state => ({
      ...state,
      streamingMessage
    }));
  }

  private handleStreamEvent(event: ChatStreamEvent, messageId: string): void {
    const chunk = event.chunk;

    switch (chunk.type) {
      case 'content':
        this.appendStreamContent(chunk.content);
        break;
      case 'toolCallStart':
        this.handleToolCallStart(chunk.id, chunk.name);
        break;
      case 'toolCallArgument':
        this.appendToolCallArgument(chunk.id, chunk.argument);
        break;
      case 'toolCallComplete':
        this.completeToolCall(chunk.id);
        break;
      case 'done':
        this.finalizeStreamingMessage(messageId);
        break;
    }
  }

  private appendStreamContent(chunk: string): void {
    this.streamingBuffer += chunk;
    this.scheduleStreamingUpdate();

    // Dispatch custom event for performance monitoring
    window.dispatchEvent(new CustomEvent('streaming-chunk-received', {
      detail: { size: chunk.length, timestamp: Date.now() }
    }));
  }

  private scheduleStreamingUpdate(): void {
    if (this.pendingUpdate) {
      return; // Already have a pending update
    }

    this.pendingUpdate = true;
    this.streamingUpdateRAF = requestAnimationFrame(() => {
      this.flushStreamingBuffer();
      this.pendingUpdate = false;
      this.streamingUpdateRAF = null;
    });
  }

  private flushStreamingBuffer(): void {
    this.store.setState(state => {
      if (state.streamingMessage && this.streamingBuffer) {
        return {
          ...state,
          streamingMessage: {
            ...state.streamingMessage,
            partialContent: this.streamingBuffer,
          }
        };
      }
      return state;
    });
  }

  private handleToolCallStart(toolId: string, toolName: string): void {
    const toolCallMessage: ChatMessage = {
      type: "toolCall",
      id: toolId,
      name: toolName,
      params: '',
      timestamp: new Date().toISOString(),
    };

    const toolCallBlock: ChatMessageBlock = {
      id: toolId,
      message: toolCallMessage,
    };

    this.store.setState(state => {
      const newToolCalls = new Map(state.toolCalls);
      newToolCalls.set(toolId, 'Running');
      return {
        ...state,
        messages: [...state.messages, toolCallBlock],
        toolCalls: newToolCalls
      };
    });
  }

  private appendToolCallArgument(toolId: string, chunk: string): void {
    this.store.setState(state => {
      const messageIndex = state.messages.findIndex(msg => msg.id === toolId && msg.message.type === 'toolCall');

      if (messageIndex !== -1) {
        const updatedMessages = [...state.messages];
        const targetMessage = updatedMessages[messageIndex];

        if (targetMessage.message.type === 'toolCall') {
          updatedMessages[messageIndex] = {
            ...targetMessage,
            message: {
              ...targetMessage.message,
              params: targetMessage.message.params + chunk,
            },
          };
        }

        return {
          ...state,
          messages: updatedMessages
        };
      }
      return state;
    });
  }

  private completeToolCall(toolId: string): void {
    this.store.setState(state => {
      const newToolCalls = new Map(state.toolCalls);
      newToolCalls.set(toolId, 'Completed');
      return {
        ...state,
        toolCalls: newToolCalls
      };
    });
  }

  private finalizeStreamingMessage(messageId: string): void {
    // Clear any pending updates and flush final content
    if (this.streamingUpdateRAF !== null) {
      cancelAnimationFrame(this.streamingUpdateRAF);
      this.streamingUpdateRAF = null;
      this.pendingUpdate = false;
    }

    this.store.setState(state => {
      if (!state.streamingMessage) return state;

      // Use the buffer content for final message to ensure all content is captured
      const finalContent = this.streamingBuffer || state.streamingMessage.partialContent;

      const finalMessage: ChatMessageBlock = {
        id: messageId,
        message: {
          type: "assistant",
          content: finalContent,
          timestamp: state.streamingMessage.message.timestamp,
        },
      };

      return {
        ...state,
        messages: [...state.messages, finalMessage],
        streamingMessage: null
      };
    });

    // Reset streaming state
    this.streamingBuffer = '';
  }

  private handleStreamError(error: Error, _messageId: string): void {
    console.error('Stream error:', error);

    const errorMessage: ChatMessage = {
      type: "error",
      message: error.message,
      timestamp: new Date().toISOString(),
    };

    const errorBlock: ChatMessageBlock = {
      id: crypto.randomUUID(),
      message: errorMessage,
    };

    this.store.setState(state => ({
      ...state,
      messages: [...state.messages, errorBlock],
      streamingMessage: null
    }));
  }


  async getChatHistory(): Promise<void> {
    try {
      const result = await commands.getChatHistory();
      if (result.status === "ok") {
        // Convert ChatMessage[] to ChatMessageBlock[]
        const messageBlocks: ChatMessageBlock[] = result.data.map(msg => ({
          id: crypto.randomUUID(),
          message: msg,
        }));
        this.store.setState(state => ({
          ...state,
          messages: messageBlocks
        }));
      } else {
        throw new Error(result.error);
      }
    } catch (error) {
      console.error("Failed to get chat history:", error);
    }
  }

  async clearChatHistory(): Promise<void> {
    try {
      const result = await commands.clearChatHistory();
      if (result.status === "ok") {
        this.store.setState(state => ({
          ...state,
          messages: [],
          streamingMessage: null
        }));
      } else {
        throw new Error(result.error);
      }
    } catch (error) {
      console.error("Failed to clear chat history:", error);
    }
  }

  // Configuration Methods
  async loadConfig(): Promise<void> {
    try {
      const result = await commands.getConfig();
      if (result.status === "ok") {
        this.store.setState(state => ({
          ...state,
          config: result.data
        }));
      } else {
        throw new Error(result.error);
      }
    } catch (error) {
      console.error("Failed to load config:", error);
    }
  }

  async saveConfig(config: AppConfig): Promise<void> {
    try {
      const result = await commands.updateConfig(config);
      if (result.status === "ok") {
        this.store.setState(state => ({
          ...state,
          config
        }));
      } else {
        throw new Error(result.error);
      }
    } catch (error) {
      console.error("Failed to save config:", error);
    }
  }

  async loadAppStatus(): Promise<void> {
    try {
      const result = await commands.getAppStatus();
      if (result.status === "ok") {
        this.store.setState(state => ({
          ...state,
          status: result.data
        }));
      } else {
        throw new Error(result.error);
      }
    } catch (error) {
      console.error("Failed to load app status:", error);
    }
  }

  async initializeAgent(
    provider: LlmProvider,
    openrouterConfig?: OpenRouterConfig,
    ollamaConfig?: OllamaConfig,
    systemPrompt?: string
  ): Promise<void> {
    try {
      const request: InitializeAgentRequest = {
        provider,
        openrouter_config: openrouterConfig || null,
        ollama_config: ollamaConfig || null,
        system_prompt: systemPrompt || null,
      };

      const result = await commands.initializeAgent(request);
      if (result.status === "error") {
        throw new Error(result.error);
      }

      // Refresh app status after successful initialization
      await this.loadAppStatus();
    } catch (error) {
      console.error("Failed to initialize agent:", error);
      throw error;
    }
  }

  async testProviderConnection(
    provider: LlmProvider,
    openrouterConfig?: OpenRouterConfig,
    ollamaConfig?: OllamaConfig
  ): Promise<boolean> {
    try {
      const result = await commands.testProviderConnection(
        provider,
        openrouterConfig || null,
        ollamaConfig || null
      );
      if (result.status === "ok") {
        return result.data;
      } else {
        throw new Error(result.error);
      }
    } catch (error) {
      console.error("Failed to test provider connection:", error);
      throw error;
    }
  }

  selectProvider(provider: LlmProvider): void {
    const state = this.store.getState();
    if (state.config) {
      const updatedConfig: AppConfig = {
        ...state.config,
        active_provider: provider,
      };
      this.saveConfig(updatedConfig);
    }
  }

  // UI State Methods
  setTheme(theme: Theme): void {
    this.store.setState(state => ({
      ...state,
      ui: {
        ...state.ui,
        theme,
      }
    }));
  }

  toggleSidebar(): void {
    this.store.setState(state => ({
      ...state,
      ui: {
        ...state.ui,
        sidebarOpen: !state.ui.sidebarOpen,
      }
    }));
  }

  toggleSettings(): void {
    this.store.setState(state => ({
      ...state,
      ui: {
        ...state.ui,
        settingsOpen: !state.ui.settingsOpen,
      }
    }));
  }

  openSettings(): void {
    this.store.setState(state => ({
      ...state,
      ui: {
        ...state.ui,
        settingsOpen: true,
      }
    }));
  }

  closeSettings(): void {
    this.store.setState(state => ({
      ...state,
      ui: {
        ...state.ui,
        settingsOpen: false,
      }
    }));
  }

  toggleCommandPalette(): void {
    this.store.setState(state => ({
      ...state,
      ui: {
        ...state.ui,
        commandPaletteOpen: !state.ui.commandPaletteOpen,
      }
    }));
  }

  // Message Management
  selectMessage(messageId: string | null): void {
    this.store.setState(state => ({
      ...state,
      selectedMessageId: messageId
    }));
  }

  toggleMessageCollapse(messageId: string): void {
    this.store.setState(state => ({
      ...state,
      messages: state.messages.map(msg =>
        msg.id === messageId
          ? { ...msg, collapsed: !msg.collapsed }
          : msg
      )
    }));
  }

  // Scroll Management
  setScrollOffset(offset: number): void {
    this.store.setState(state => ({
      ...state,
      scroll: {
        ...state.scroll,
        offset,
        atBottom: offset === 0,
      }
    }));
  }

  enableAutoScroll(): void {
    this.store.setState(state => ({
      ...state,
      scroll: {
        ...state.scroll,
        autoScroll: true,
      }
    }));
  }

  disableAutoScroll(): void {
    this.store.setState(state => ({
      ...state,
      scroll: {
        ...state.scroll,
        autoScroll: false,
      }
    }));
  }

  scrollToBottom(): void {
    this.setScrollOffset(0);
  }

  // App initialization
  async init(): Promise<void> {
    try {
      // Load configuration and status
      await this.loadConfig();
      await this.loadAppStatus();

      // Get the current config from store
      const { config } = this.store.getState();

      if (config) {
        // Try to initialize agent with existing config
        try {
          const openrouterConfig = config.active_provider === 'OpenRouter' ? config.openrouter_config : undefined;
          const ollamaConfig = config.active_provider === 'Ollama' ? config.ollama_config : undefined;

          await this.initializeAgent(
            config.active_provider,
            openrouterConfig,
            ollamaConfig
          );

          console.log('Agent initialized successfully with saved config');
        } catch (error) {
          console.warn('Failed to initialize agent with saved config:', error);
          // Don't throw here - we'll let the user configure settings manually
        }
      }
    } catch (error) {
      console.error('Failed to initialize app:', error);
    }
  }
}

// Factory function to create AppActions with proper dependency injection
export function createAppActions(
  store: ZustandStore<AppState>,
  createChannel: ChannelFactory,
): AppActions {
  return new AppActions(store, createChannel);
}

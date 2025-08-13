import type { 
  ChatMessage, 
  SendMessageRequest,
  ToolDiscoveryEvent,
  LlmProvider,
  AppConfig,
  InitializeAgentRequest,
  ChatStreamEvent,
  ExecuteToolCallRequest,
  OpenRouterConfig,
  OllamaConfig
} from "../generated/bindings";
import { commands } from "../generated/bindings";
import type { AppState, ZustandStore } from "./store";
import type { 
  ChatMessageBlock, 
  StreamingMessageBlock, 
  ChannelFactory,
  UIState,
  ScrollState,
  Theme
} from "../types/ui";

export class AppActions {
  private streamingBuffer = '';
  private streamingMessageId: string | null = null;
  private streamingUpdateRAF: number | null = null;
  private pendingUpdate = false;

  constructor(
    private store: ZustandStore<AppState>,
    private createChannel: ChannelFactory,
  ) {}

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

    this.store.getState().addMessage(userBlock);
  }

  private startStreamingMessage(id: string): void {
    this.streamingMessageId = id;
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

    this.store.getState().setStreamingMessage(streamingMessage);
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
    const state = this.store.getState();
    if (state.streamingMessage && this.streamingBuffer) {
      const updatedStreaming: StreamingMessageBlock = {
        ...state.streamingMessage,
        partialContent: this.streamingBuffer,
      };
      state.setStreamingMessage(updatedStreaming);
    }
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

    const state = this.store.getState();
    state.addMessage(toolCallBlock);
    state.updateToolCallState(toolId, 'Running');
  }

  private appendToolCallArgument(toolId: string, chunk: string): void {
    const state = this.store.getState();
    const messageIndex = state.messages.findIndex(msg => msg.id === toolId && msg.message.type === 'toolCall');
    
    if (messageIndex !== -1) {
      const updatedMessages = [...state.messages];
      const targetMessage = updatedMessages[messageIndex];
      
      updatedMessages[messageIndex] = {
        ...targetMessage,
        message: {
          ...targetMessage.message,
          params: targetMessage.message.params + chunk,
        },
      };

      state.setMessages(updatedMessages);
    }
  }

  private completeToolCall(toolId: string): void {
    this.store.getState().updateToolCallState(toolId, 'Completed');
  }

  private finalizeStreamingMessage(messageId: string): void {
    // Clear any pending updates and flush final content
    if (this.streamingUpdateRAF !== null) {
      cancelAnimationFrame(this.streamingUpdateRAF);
      this.streamingUpdateRAF = null;
      this.pendingUpdate = false;
    }
    
    const state = this.store.getState();
    if (!state.streamingMessage) return;

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

    state.addMessage(finalMessage);
    state.setStreamingMessage(null);
    
    // Reset streaming state
    this.streamingBuffer = '';
    this.streamingMessageId = null;
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

    const state = this.store.getState();
    state.addMessage(errorBlock);
    state.setStreamingMessage(null);
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
        this.store.getState().setMessages(messageBlocks);
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
        const state = this.store.getState();
        state.setMessages([]);
        state.setStreamingMessage(null);
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
        this.store.getState().setConfig(result.data);
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
        this.store.getState().setConfig(config);
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
        this.store.getState().setStatus(result.data);
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

  async executeToolCall(toolName: string, toolParams: any): Promise<string> {
    try {
      const request: ExecuteToolCallRequest = {
        tool_name: toolName,
        tool_params: JSON.stringify(toolParams),
      };

      const result = await commands.executeToolCall(request);
      if (result.status === "ok") {
        return result.data;
      } else {
        throw new Error(result.error);
      }
    } catch (error) {
      console.error("Failed to execute tool call:", error);
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

  async discoverTools(): Promise<void> {
    const channel = this.createChannel<ToolDiscoveryEvent>((event) => {
      this.handleToolDiscoveryEvent(event);
    });

    try {
      // This would be implemented based on your tool discovery needs
      console.log('Tool discovery started with channel:', channel.id);
    } catch (error) {
      console.error('Tool discovery failed:', error);
    }
  }

  private handleToolDiscoveryEvent(event: ToolDiscoveryEvent): void {
    switch (event.type) {
      case 'discovered':
        console.log('Tool discovered:', event.tool);
        // Update status with new tool
        break;

      case 'complete':
        console.log(`Tool discovery complete: ${event.count} tools found`);
        break;

      case 'error':
        console.error('Tool discovery error:', event.message);
        break;
    }
  }

  // UI State Methods
  setTheme(theme: Theme): void {
    const state = this.store.getState();
    const updatedUI: UIState = {
      ...state.ui,
      theme,
    };
    state.updateUI(updatedUI);
  }

  toggleSidebar(): void {
    const state = this.store.getState();
    const updatedUI: UIState = {
      ...state.ui,
      sidebarOpen: !state.ui.sidebarOpen,
    };
    state.updateUI(updatedUI);
  }

  toggleSettings(): void {
    const state = this.store.getState();
    const updatedUI: UIState = {
      ...state.ui,
      settingsOpen: !state.ui.settingsOpen,
    };
    state.updateUI(updatedUI);
  }

  openSettings(): void {
    const state = this.store.getState();
    const updatedUI: UIState = {
      ...state.ui,
      settingsOpen: true,
    };
    state.updateUI(updatedUI);
  }

  closeSettings(): void {
    const state = this.store.getState();
    const updatedUI: UIState = {
      ...state.ui,
      settingsOpen: false,
    };
    state.updateUI(updatedUI);
  }

  toggleCommandPalette(): void {
    const state = this.store.getState();
    const updatedUI: UIState = {
      ...state.ui,
      commandPaletteOpen: !state.ui.commandPaletteOpen,
    };
    state.updateUI(updatedUI);
  }

  // Message Management
  selectMessage(messageId: string | null): void {
    this.store.getState().setSelectedMessage(messageId);
  }

  toggleMessageCollapse(messageId: string): void {
    const state = this.store.getState();
    const updatedMessages = state.messages.map(msg => 
      msg.id === messageId 
        ? { ...msg, collapsed: !msg.collapsed }
        : msg
    );
    state.setMessages(updatedMessages);
  }

  // Scroll Management
  setScrollOffset(offset: number): void {
    const state = this.store.getState();
    const updatedScroll: ScrollState = {
      ...state.scroll,
      offset,
      atBottom: offset === 0,
    };
    state.updateScroll(updatedScroll);
  }

  enableAutoScroll(): void {
    const state = this.store.getState();
    const updatedScroll: ScrollState = {
      ...state.scroll,
      autoScroll: true,
    };
    state.updateScroll(updatedScroll);
  }

  disableAutoScroll(): void {
    const state = this.store.getState();
    const updatedScroll: ScrollState = {
      ...state.scroll,
      autoScroll: false,
    };
    state.updateScroll(updatedScroll);
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
      const config = this.store.getState().config;
      
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

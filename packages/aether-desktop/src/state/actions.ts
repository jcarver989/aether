import { commands } from "../generated/bindings";
import { AppState, ZustandStore, ChatMessageBlock, StreamingMessageBlock } from "./store";
import { Channel } from "@tauri-apps/api/core";
import type { 
  ChatMessage, 
  StreamEvent, 
  SendMessageRequest,
  SendMessageResponse 
} from "../generated/bindings";

export class AppActions {
  constructor(
    private store: ZustandStore<AppState>,
    private tauriCommands: typeof commands,
    private createChannel: <T>(onMessage: (message: T) => void) => Channel<T>,
  ) {}

  async sendMessage(content: string): Promise<void> {
    const messageId = crypto.randomUUID();
    
    // Add user message to store immediately
    const userMessage: ChatMessage = {
      type: "User",
      content,
      timestamp: Date.now(), // Will need to adjust this based on actual SystemTime format
    };
    
    const userBlock: ChatMessageBlock = {
      id: messageId,
      message: userMessage,
    };

    this.store.setState((state) => ({
      ...state,
      messages: [...state.messages, userBlock],
    }));

    // Create streaming message block for assistant response
    const assistantMessageId = crypto.randomUUID();
    const streamingBlock: StreamingMessageBlock = {
      id: assistantMessageId,
      message: {
        type: "Assistant",
        content: "",
        timestamp: Date.now(),
      },
      isStreaming: true,
      partialContent: "",
    };

    this.store.setState((state) => ({
      ...state,
      streamingMessage: streamingBlock,
    }));

    // Set up streaming channel
    const channel = this.createChannel<StreamEvent>((event) => {
      this.handleStreamEvent(event, assistantMessageId);
    });

    try {
      const request: SendMessageRequest = {
        content,
        message_id: messageId,
      };

      await this.tauriCommands.sendMessage(request, channel);
    } catch (error) {
      console.error("Failed to send message:", error);
      // Add error handling - could add error message to chat
    }
  }

  private handleStreamEvent(event: StreamEvent, messageId: string): void {
    const { chunk } = event;

    this.store.setState((state) => {
      const currentStreaming = state.streamingMessage;
      if (!currentStreaming || currentStreaming.id !== messageId) {
        return state;
      }

      switch (chunk.type) {
        case "Content":
          return {
            ...state,
            streamingMessage: {
              ...currentStreaming,
              partialContent: currentStreaming.partialContent + chunk.content,
              message: {
                ...currentStreaming.message,
                content: currentStreaming.partialContent + chunk.content,
              },
            },
          };

        case "Done":
          // Move streaming message to regular messages
          const finalMessage: ChatMessageBlock = {
            id: currentStreaming.id,
            message: currentStreaming.message,
          };
          
          return {
            ...state,
            messages: [...state.messages, finalMessage],
            streamingMessage: null,
          };

        case "ToolCallStart":
          // TODO: Handle tool call start
          return state;

        case "ToolCallArgument":
          // TODO: Handle tool call argument
          return state;

        case "ToolCallComplete":
          // TODO: Handle tool call complete
          return state;

        default:
          return state;
      }
    });
  }

  async getChatHistory(): Promise<void> {
    try {
      const history = await this.tauriCommands.getChatHistory();
      // TODO: Convert history to ChatMessageBlocks and update store
    } catch (error) {
      console.error("Failed to get chat history:", error);
    }
  }

  async clearChatHistory(): Promise<void> {
    try {
      await this.tauriCommands.clearChatHistory();
      this.store.setState((state) => ({
        ...state,
        messages: [],
        streamingMessage: null,
      }));
    } catch (error) {
      console.error("Failed to clear chat history:", error);
    }
  }
}

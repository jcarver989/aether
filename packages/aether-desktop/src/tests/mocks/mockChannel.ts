import { ChannelFactory } from "../../types/ui";
import { Channel as TauriChannel } from "@tauri-apps/api/core";

export class MockChannel<T> {
  id = crypto.randomUUID();
  onmessage?: (message: T) => void;
  private messages: T[] = [];
  private closed = false;

  constructor(onMessage?: (message: T) => void) {
    this.onmessage = onMessage;
  }

  async send(message: T): Promise<void> {
    if (this.closed) {
      throw new Error(`Channel ${this.id} is closed`);
    }
    
    this.messages.push(message);
    
    // Simulate async behavior
    setTimeout(() => {
      if (!this.closed && this.onmessage) {
        this.onmessage(message);
      }
    }, 0);
  }

  close(): void {
    this.closed = true;
    this.onmessage = undefined;
  }

  // Test helper methods
  async simulateStream(messages: T[], delayMs = 10): Promise<void> {
    for (const message of messages) {
      if (this.closed) break;
      
      await this.send(message);
      if (delayMs > 0) {
        await new Promise(resolve => setTimeout(resolve, delayMs));
      }
    }
  }

  simulateMessage(message: T): void {
    if (!this.closed && this.onmessage) {
      this.onmessage(message);
    }
  }

  getMessages(): T[] {
    return [...this.messages];
  }

  clear(): void {
    this.messages = [];
  }

  isClosed(): boolean {
    return this.closed;
  }
}

export function createMockChannelFactory(): ChannelFactory {
  const channels = new Map<string, MockChannel<any>>();
  
  return function createChannel<T>(onMessage: (message: T) => void): TauriChannel<T> {
    const channel = new MockChannel<T>(onMessage);
    channels.set(channel.id, channel);
    return channel as any as TauriChannel<T>; // Cast for testing purposes
  };
}

// Helper for tests to get created channels
export function createMockChannelFactoryWithTracking(): {
  factory: ChannelFactory;
  getChannels: () => MockChannel<any>[];
  getChannel: <T>(id: string) => MockChannel<T> | undefined;
  clearChannels: () => void;
} {
  const channels = new Map<string, MockChannel<any>>();
  
  const factory: ChannelFactory = function createChannel<T>(onMessage: (message: T) => void): TauriChannel<T> {
    const channel = new MockChannel<T>(onMessage);
    channels.set(channel.id, channel);
    return channel as any as TauriChannel<T>; // Cast for testing purposes
  };
  
  return {
    factory,
    getChannels: () => Array.from(channels.values()),
    getChannel: <T>(id: string) => channels.get(id) as MockChannel<T> | undefined,
    clearChannels: () => channels.clear(),
  };
}
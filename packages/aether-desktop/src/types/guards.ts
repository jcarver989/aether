import type { ChatMessage, StreamEvent, ToolDiscoveryEvent, StreamChunk } from "../generated/bindings";

// Type guards for chat messages
export function isSystemMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'system' }> {
  return message.type === 'system';
}

export function isUserMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'user' }> {
  return message.type === 'user';
}

export function isAssistantMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'assistant' }> {
  return message.type === 'assistant';
}

export function isAssistantStreamingMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'assistantStreaming' }> {
  return message.type === 'assistantStreaming';
}

export function isToolMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'tool' }> {
  return message.type === 'tool';
}

export function isToolCallMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'toolCall' }> {
  return message.type === 'toolCall';
}

export function isToolResultMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'toolResult' }> {
  return message.type === 'toolResult';
}

export function isErrorMessage(message: ChatMessage): message is Extract<ChatMessage, { type: 'error' }> {
  return message.type === 'error';
}

// Type guards for stream events
export function isStreamStartEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'start' }> {
  return event.type === 'start';
}

export function isStreamContentEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'content' }> {
  return event.type === 'content';
}

export function isStreamToolCallStartEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'toolCallStart' }> {
  return event.type === 'toolCallStart';
}

export function isStreamToolCallArgumentEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'toolCallArgument' }> {
  return event.type === 'toolCallArgument';
}

export function isStreamToolCallCompleteEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'toolCallComplete' }> {
  return event.type === 'toolCallComplete';
}

export function isStreamDoneEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'done' }> {
  return event.type === 'done';
}

export function isStreamErrorEvent(event: StreamEvent): event is Extract<StreamEvent, { type: 'error' }> {
  return event.type === 'error';
}

// Type guards for tool discovery events
export function isToolDiscoveredEvent(event: ToolDiscoveryEvent): event is Extract<ToolDiscoveryEvent, { type: 'discovered' }> {
  return event.type === 'discovered';
}

export function isToolDiscoveryCompleteEvent(event: ToolDiscoveryEvent): event is Extract<ToolDiscoveryEvent, { type: 'complete' }> {
  return event.type === 'complete';
}

export function isToolDiscoveryErrorEvent(event: ToolDiscoveryEvent): event is Extract<ToolDiscoveryEvent, { type: 'error' }> {
  return event.type === 'error';
}

// Type guards for stream chunks
export function isContentChunk(chunk: StreamChunk): chunk is Extract<StreamChunk, { type: 'content' }> {
  return chunk.type === 'content';
}

export function isToolCallStartChunk(chunk: StreamChunk): chunk is Extract<StreamChunk, { type: 'toolCallStart' }> {
  return chunk.type === 'toolCallStart';
}

export function isToolCallArgumentChunk(chunk: StreamChunk): chunk is Extract<StreamChunk, { type: 'toolCallArgument' }> {
  return chunk.type === 'toolCallArgument';
}

export function isToolCallCompleteChunk(chunk: StreamChunk): chunk is Extract<StreamChunk, { type: 'toolCallComplete' }> {
  return chunk.type === 'toolCallComplete';
}

export function isDoneChunk(chunk: StreamChunk): chunk is Extract<StreamChunk, { type: 'done' }> {
  return chunk.type === 'done';
}

// Utility functions for message handling
export function getMessageContent(message: ChatMessage): string {
  switch (message.type) {
    case 'system':
    case 'user':
    case 'assistant':
    case 'assistantStreaming':
      return message.content;
    case 'tool':
    case 'toolResult':
      return message.content;
    case 'toolCall':
      return `Tool: ${message.name}\nParams: ${message.params}`;
    case 'error':
      return `Error: ${message.message}`;
    default:
      return '';
  }
}

export function getMessageTimestamp(message: ChatMessage): Date {
  return new Date(message.timestamp);
}

export function isStreamingMessage(message: ChatMessage): boolean {
  return message.type === 'assistantStreaming';
}
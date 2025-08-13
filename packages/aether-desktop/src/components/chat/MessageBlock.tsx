import React, { memo, useCallback } from 'react';
import { ChatMessageBlock, StreamingMessageBlock } from '@/types';
import { SystemBlock } from './blocks/SystemBlock';
import { UserBlock } from './blocks/UserBlock';
import { AssistantBlock } from './blocks/AssistantBlock';
import { StreamingAssistantBlock } from './blocks/StreamingAssistantBlock';
import { ToolCallBlock } from './blocks/ToolCallBlock';
import { ToolResultBlock } from './blocks/ToolResultBlock';
import { ErrorBlock } from './blocks/ErrorBlock';

interface MessageBlockProps {
  block: ChatMessageBlock | StreamingMessageBlock;
  onCopy?: (content: string) => void;
  className?: string;
}

export const MessageBlock: React.FC<MessageBlockProps> = memo(({
  block,
  onCopy,
  className,
}) => {
  const handleCopy = useCallback(() => {
    if (onCopy) {
      const content = 'partialContent' in block 
        ? block.partialContent 
        : getMessageContent(block.message);
      onCopy(content);
    }
  }, [onCopy, block]);

  // Handle streaming messages
  if ('isStreaming' in block && block.isStreaming) {
    return (
      <StreamingAssistantBlock 
        block={block as StreamingMessageBlock}
        className={className}
      />
    );
  }

  // Handle regular messages
  const regularBlock = block as ChatMessageBlock;
  
  switch (regularBlock.message.type) {
    case 'system':
      return (
        <SystemBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    case 'user':
      return (
        <UserBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    case 'assistant':
      return (
        <AssistantBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    case 'toolCall':
      return (
        <ToolCallBlock 
          block={regularBlock}
          className={className}
        />
      );
      
    case 'toolResult':
      return (
        <ToolResultBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    case 'error':
      return (
        <ErrorBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    default:
      return (
        <div className="rounded-xl border border-destructive/20 bg-destructive/5 p-4">
          <div className="text-sm text-destructive">
            Unknown message type: {(regularBlock.message as any).type}
          </div>
        </div>
      );
  }
});

function getMessageContent(message: any): string {
  switch (message.type) {
    case 'system':
    case 'user':
    case 'assistant':
      return message.content;
    case 'toolCall':
      return JSON.stringify(message.params, null, 2);
    case 'toolResult':
      return message.content;
    case 'error':
      return message.message;
    default:
      return '';
  }
}
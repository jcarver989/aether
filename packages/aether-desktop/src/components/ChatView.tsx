import React, { useEffect, useRef, memo, useCallback } from 'react';
import { MessageBlock } from './chat/MessageBlock';
import { useAppContext } from '@/hooks/useAppContext';
import { useRAFOptimizedSelectors } from '@/hooks/useRAFStreamingOptimization';
import { cn } from '@/lib/utils';
import toast from 'react-hot-toast';

interface ChatViewProps {
  className?: string;
}

export const ChatView: React.FC<ChatViewProps> = memo(({ className }) => {
  const { actions } = useAppContext();
  const scrollRef = useRef<HTMLDivElement>(null);
  
  const { messages, streamingMessage, autoScroll } = useRAFOptimizedSelectors();
  
  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, streamingMessage, autoScroll]);

  const handleCopy = useCallback(async (content: string) => {
    try {
      await navigator.clipboard.writeText(content);
      toast.success('Copied to clipboard!');
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
      toast.error('Failed to copy to clipboard');
    }
  }, []);

  const handleScroll = useCallback((event: React.UIEvent<HTMLDivElement>) => {
    const target = event.target as HTMLDivElement;
    const isAtBottom = target.scrollHeight - target.scrollTop <= target.clientHeight + 10;
    
    if (isAtBottom && !autoScroll) {
      actions.enableAutoScroll();
    } else if (!isAtBottom && autoScroll) {
      actions.disableAutoScroll();
    }
  }, [actions, autoScroll]);

  return (
    <div 
      ref={scrollRef}
      className={cn("flex-1 px-6 py-4 overflow-y-auto overflow-x-hidden", className)}
      onScroll={handleScroll}
    >
      <div className="space-y-4 break-words max-w-4xl mx-auto">
        {messages.map((block) => (
          <MessageBlock
            key={block.id}
            block={block}
            onCopy={handleCopy}
          />
        ))}
        
        {streamingMessage && (
          <MessageBlock
            block={streamingMessage}
            onCopy={handleCopy}
          />
        )}
        
        {messages.length === 0 && !streamingMessage && (
          <div className="flex items-center justify-center h-full">
            <div className="text-center max-w-md">
              <div className="mb-6">
                <div className="h-16 w-16 rounded-2xl bg-gradient-to-br from-primary to-primary/80 flex items-center justify-center mx-auto mb-4">
                  <span className="text-primary-foreground font-bold text-2xl">A</span>
                </div>
              </div>
              <h2 className="text-2xl font-semibold mb-3 text-foreground">Welcome to Aether</h2>
              <p className="text-muted-foreground text-sm leading-relaxed">
                Your AI coding assistant is ready to help. Start a conversation by typing a message below.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});
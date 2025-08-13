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
            <div className="text-center max-w-md border-2 border-border p-8 bg-card/50 shadow-retro-inset">
              <div className="mb-6">
                <div className="h-16 w-16 border-3 border-primary bg-background flex items-center justify-center mx-auto mb-4 shadow-retro font-mono">
                  <span className="text-primary font-bold text-2xl tracking-wider animate-pulse-subtle">[A]</span>
                </div>
              </div>
              <h2 className="text-2xl font-bold mb-3 text-foreground uppercase tracking-wider font-mono">
                :: AETHER TERMINAL ::
              </h2>
              <p className="text-muted-foreground text-sm leading-relaxed font-mono uppercase tracking-wide">
                &gt; SYSTEM READY<br/>
                &gt; AWAITING INPUT...
              </p>
              <div className="mt-4 text-xs text-primary/60 font-mono">
                [COGNITIVE PROCESSING UNIT ONLINE]
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});
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
            <div className="text-center max-w-lg border border-border/40 p-10 bg-gradient-to-br from-card/30 to-background/50 shadow-hologram backdrop-blur-sm">
              <div className="mb-8">
                <div className="h-20 w-20 border border-primary/40 bg-gradient-to-br from-primary/10 to-accent/10 flex items-center justify-center mx-auto mb-6 shadow-neon font-mono relative overflow-hidden">
                  <span className="text-primary font-light text-3xl tracking-widest animate-pulse-glow relative z-10">A</span>
                  <div className="absolute inset-0 bg-gradient-to-r from-transparent via-primary/20 to-transparent animate-shimmer"></div>
                </div>
              </div>
              <h2 className="text-3xl font-light mb-4 text-foreground tracking-[0.2em] font-mono">
                AETHER
              </h2>
              <div className="text-sm font-mono text-muted-foreground/70 space-y-2 leading-relaxed">
                <div className="flex items-center justify-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-primary animate-pulse-glow"></div>
                  <span>Neural interface initialized</span>
                </div>
                <div className="flex items-center justify-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-accent animate-pulse-glow"></div>
                  <span>Cognitive systems online</span>
                </div>
                <div className="flex items-center justify-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-primary animate-pulse-glow"></div>
                  <span>Ready for interaction</span>
                </div>
              </div>
              <div className="mt-6 text-xs text-primary/40 font-mono font-light tracking-wider">
                Advanced Intelligence • Quantum Processing
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});
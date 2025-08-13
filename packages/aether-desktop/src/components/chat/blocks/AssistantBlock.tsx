import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { MarkdownRenderer } from '../MarkdownRenderer';

interface AssistantBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const AssistantBlock: React.FC<AssistantBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  if (block.message.type !== 'assistant') {
    throw new Error('AssistantBlock can only render assistant messages');
  }

  return (
    <div className={cn(
      "group border border-accent/30 bg-gradient-to-br from-muted/15 to-accent/5 p-5 font-mono backdrop-blur-sm",
      "transition-all duration-200 hover:border-accent/50 hover:from-muted/25 hover:to-accent/10 hover:shadow-neon-subtle",
      "shadow-hologram relative overflow-hidden",
      className
    )}>
      {/* Accent edge glow */}
      <div className="absolute left-0 top-0 h-full w-0.5 bg-gradient-to-b from-accent/50 via-accent/20 to-transparent"></div>
      
      <BlockHeader
        title="AETHER"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-accent font-medium tracking-wide"
      />
      
      <div className="text-sm text-foreground/95 mt-4 prose prose-sm prose-invert max-w-none font-mono font-light">
        <MarkdownRenderer content={block.message.content} />
      </div>
    </div>
  );
};
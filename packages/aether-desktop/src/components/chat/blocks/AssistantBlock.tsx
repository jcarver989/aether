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
      "group rounded-xl border border-border/50 bg-muted/30 p-6",
      "transition-all duration-200 hover:border-border hover:bg-muted/50 hover:shadow-lg hover:shadow-accent/5",
      "backdrop-blur-sm",
      className
    )}>
      <BlockHeader
        title="Assistant"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-foreground font-medium"
      />
      
      <div className="text-sm text-foreground/90 mt-3 prose prose-sm prose-invert max-w-none">
        <MarkdownRenderer content={block.message.content} />
      </div>
    </div>
  );
};
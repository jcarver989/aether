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
      "group border-2 border-foreground/40 bg-muted/20 p-4 font-mono",
      "transition-all duration-100 hover:border-foreground hover:bg-muted/40 hover:shadow-terminal-glow",
      "shadow-retro-inset",
      className
    )}>
      <BlockHeader
        title="[AETHER]"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-foreground font-bold uppercase tracking-wider"
      />
      
      <div className="text-sm text-foreground/95 mt-3 prose prose-sm prose-invert max-w-none font-mono">
        <MarkdownRenderer content={block.message.content} />
      </div>
    </div>
  );
};
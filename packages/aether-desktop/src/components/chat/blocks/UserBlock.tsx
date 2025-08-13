import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';

interface UserBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const UserBlock: React.FC<UserBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  if (block.message.type !== 'user') {
    throw new Error('UserBlock can only render user messages');
  }

  return (
    <div className={cn(
      "group border-2 border-primary/60 bg-card/30 p-4 font-mono",
      "transition-all duration-100 hover:border-primary hover:bg-card/50 hover:shadow-retro",
      "shadow-retro-inset",
      className
    )}>
      <BlockHeader
        title="[USER]"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-primary font-bold uppercase tracking-wider"
      />
      
      <div className="text-sm text-card-foreground whitespace-pre-wrap leading-relaxed mt-3 font-mono">
        &gt; {block.message.content}
      </div>
    </div>
  );
};
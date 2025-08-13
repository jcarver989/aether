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
      "group rounded-xl border border-border/50 bg-card/50 p-6",
      "transition-all duration-200 hover:border-border hover:bg-card/80 hover:shadow-lg hover:shadow-primary/5",
      "backdrop-blur-sm",
      className
    )}>
      <BlockHeader
        title="You"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-primary font-medium"
      />
      
      <div className="text-sm text-card-foreground/90 whitespace-pre-wrap leading-relaxed mt-3">
        {block.message.content}
      </div>
    </div>
  );
};
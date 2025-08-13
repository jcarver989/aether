import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';

interface SystemBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const SystemBlock: React.FC<SystemBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  if (block.message.type !== 'system') {
    throw new Error('SystemBlock can only render system messages');
  }

  return (
    <div className={cn(
      "group rounded-xl border border-muted/50 bg-muted/20 p-6",
      "transition-all duration-200 hover:border-muted hover:bg-muted/30 hover:shadow-lg hover:shadow-muted/10",
      "backdrop-blur-sm",
      className
    )}>
      <BlockHeader
        title="System"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-muted-foreground font-medium"
      />
      
      <div className="text-sm text-muted-foreground/90 whitespace-pre-wrap mt-3 leading-relaxed">
        {block.message.content}
      </div>
    </div>
  );
};
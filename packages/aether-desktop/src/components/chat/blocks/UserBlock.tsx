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
      "group border border-primary/30 bg-gradient-to-br from-card/20 to-primary/5 p-5 font-mono backdrop-blur-sm",
      "transition-all duration-200 hover:border-primary/60 hover:from-card/30 hover:to-primary/10 hover:shadow-neon-subtle",
      "shadow-hologram relative overflow-hidden",
      className
    )}>
      {/* Subtle edge glow */}
      <div className="absolute left-0 top-0 h-full w-0.5 bg-gradient-to-b from-primary/50 via-primary/20 to-transparent"></div>
      
      <BlockHeader
        title="USER"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-primary font-medium tracking-wide"
      />
      
      <div className="text-sm text-card-foreground/95 whitespace-pre-wrap leading-relaxed mt-4 font-mono font-light">
        {block.message.content}
      </div>
    </div>
  );
};
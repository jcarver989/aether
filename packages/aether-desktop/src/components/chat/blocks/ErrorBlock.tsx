import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { AlertCircle } from 'lucide-react';

interface ErrorBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const ErrorBlock: React.FC<ErrorBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  if (block.message.type !== 'error') {
    throw new Error('ErrorBlock can only render error messages');
  }

  return (
    <div className={cn(
      "group rounded-xl border border-destructive/50 bg-destructive/10 p-6",
      "transition-all duration-200 hover:border-destructive hover:bg-destructive/15 hover:shadow-lg hover:shadow-destructive/10",
      "backdrop-blur-sm",
      className
    )}>
      <BlockHeader
        title="Error"
        timestamp={block.message.timestamp}
        indicator={<AlertCircle className="w-4 h-4 text-destructive" />}
        onCopy={onCopy}
        className="text-destructive font-medium"
      />
      
      <div className="text-sm text-destructive/90 font-mono mt-3 leading-relaxed">
        {block.message.message}
      </div>
    </div>
  );
};
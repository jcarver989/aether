import React, { useState } from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { ChevronDown, ChevronUp } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface ToolResultBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const ToolResultBlock: React.FC<ToolResultBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  const [expanded, setExpanded] = useState(false);
  
  if (block.message.type !== 'toolResult') {
    throw new Error('ToolResultBlock can only render toolResult messages');
  }

  const isLongContent = block.message.content.length > 500;
  const displayContent = expanded || !isLongContent 
    ? block.message.content 
    : block.message.content.slice(0, 500) + '...';

  // Tool results are generally successful unless there's an explicit error
  const success = true; // We could add a success field to the type later

  return (
    <div className={cn(
      "group rounded-xl border border-secondary/50 bg-secondary/30 p-6",
      "transition-all duration-200 hover:border-secondary hover:bg-secondary/40 hover:shadow-lg hover:shadow-secondary/10",
      "backdrop-blur-sm",
      className
    )}>
      <BlockHeader
        title={`Tool Result ${success ? '✓' : '✗'}`}
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-secondary-foreground font-medium"
      />
      
      <div className="text-sm whitespace-pre-wrap text-secondary-foreground/90 mt-3 font-mono leading-relaxed">
        {displayContent}
      </div>
      
      {isLongContent && (
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setExpanded(!expanded)}
          className="mt-3 h-auto p-2 text-xs text-muted-foreground hover:text-foreground hover:bg-background/20"
        >
          {expanded ? (
            <>
              <ChevronUp className="w-3 h-3 mr-1" />
              Show Less
            </>
          ) : (
            <>
              <ChevronDown className="w-3 h-3 mr-1" />
              Show More
            </>
          )}
        </Button>
      )}
    </div>
  );
};
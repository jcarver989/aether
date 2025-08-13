import React, { memo, useMemo, useDeferredValue } from 'react';
import { BlockHeader } from '../BlockHeader';
import { StreamingIndicator } from '../StreamingIndicator';
import { StreamingMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { MarkdownRenderer } from '../MarkdownRenderer';

interface StreamingAssistantBlockProps {
  block: StreamingMessageBlock;
  className?: string;
}

export const StreamingAssistantBlock: React.FC<StreamingAssistantBlockProps> = memo(({
  block,
  className,
}) => {
  // Use deferred value for non-critical content updates to prevent blocking urgent updates
  const deferredContent = useDeferredValue(block.partialContent);
  
  const memoizedHeader = useMemo(() => (
    <BlockHeader
      title="Assistant"
      timestamp={block.message.timestamp}
      indicator={<StreamingIndicator />}
      className="text-foreground font-medium"
    />
  ), [block.message.timestamp]);

  const memoizedContent = useMemo(() => (
    <div style={{ display: 'inline' }}>
      <MarkdownRenderer content={deferredContent} />
    </div>
  ), [deferredContent]);

  return (
    <div className={cn(
      "group rounded-xl border border-border/50 bg-muted/30 p-6",
      "transition-all duration-200 hover:border-border hover:bg-muted/50 hover:shadow-lg hover:shadow-accent/5",
      "backdrop-blur-sm animate-pulse-subtle",
      className
    )}>
      {memoizedHeader}
      
      <div className="text-sm text-foreground/90 mt-3 prose prose-sm prose-invert max-w-none">
        {memoizedContent}
        <span className="inline-block w-2 h-4 bg-primary animate-pulse ml-1 align-text-top" />
      </div>
    </div>
  );
}, (prevProps, nextProps) => {
  // Only re-render if the content has actually changed
  return (
    prevProps.block.partialContent === nextProps.block.partialContent &&
    prevProps.block.message.timestamp === nextProps.block.message.timestamp &&
    prevProps.className === nextProps.className
  );
});
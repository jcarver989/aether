import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ToolStateIndicator } from '../StreamingIndicator';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { useAppContext } from '@/hooks/useAppContext';
import { useSelector } from '@/hooks/useSelector';
import type { ToolCallState } from '@/generated/bindings';

interface ToolCallBlockProps {
  block: ChatMessageBlock;
  onToggle?: () => void;
  className?: string;
}

export const ToolCallBlock: React.FC<ToolCallBlockProps> = ({
  block,
  onToggle,
  className,
}) => {
  const { actions } = useAppContext();
  
  // Helper function to convert backend ToolCallState to UI state
  const convertToolCallState = (state: ToolCallState | 'pending'): 'pending' | 'running' | 'completed' | 'failed' => {
    if (typeof state === 'string' && state === 'pending') return 'pending';
    switch (state) {
      case 'Pending': return 'pending';
      case 'Running': return 'running';
      case 'Completed': return 'completed';
      case 'Failed': return 'failed';
      default: return 'pending';
    }
  };

  const toolState = useSelector(state => {
    if (block.message.type === 'toolCall') {
      const backendState = state.toolCalls.get(block.message.id) || 'pending';
      return convertToolCallState(backendState);
    }
    return 'pending';
  });

  if (block.message.type !== 'toolCall') {
    throw new Error('ToolCallBlock can only render toolCall messages');
  }

  const handleToggle = () => {
    if (onToggle) {
      onToggle();
    } else {
      actions.toggleMessageCollapse(block.id);
    }
  };

  return (
    <div className={cn(
      "group rounded-xl border border-accent/50 bg-accent/20 p-6",
      "transition-all duration-200 hover:border-accent hover:bg-accent/30 hover:shadow-lg hover:shadow-accent/10",
      "backdrop-blur-sm",
      toolState === 'running' && "animate-pulse-subtle",
      className
    )}>
      <BlockHeader
        title={`Tool: ${block.message.name}`}
        timestamp={block.message.timestamp}
        indicator={<ToolStateIndicator state={toolState} />}
        collapsible
        collapsed={block.collapsed}
        onToggle={handleToggle}
        className="text-accent-foreground font-medium"
      />
      
      {!block.collapsed && (
        <div className="mt-4">
          <div className="text-xs text-muted-foreground font-medium mb-3 uppercase tracking-wide">Parameters</div>
          <pre className="text-xs bg-background/40 border border-border/50 rounded-lg p-3 overflow-x-auto font-mono text-foreground/80">
            {block.message.params}
          </pre>
        </div>
      )}
    </div>
  );
};
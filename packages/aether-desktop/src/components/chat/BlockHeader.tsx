import React from 'react';
import { ChevronDown, ChevronRight, Copy, MoreHorizontal } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

interface BlockHeaderProps {
  title: string;
  timestamp?: Date | string;
  indicator?: React.ReactNode;
  collapsible?: boolean;
  collapsed?: boolean;
  onToggle?: () => void;
  onCopy?: () => void;
  className?: string;
}

export const BlockHeader: React.FC<BlockHeaderProps> = ({
  title,
  timestamp,
  indicator,
  collapsible = false,
  collapsed = false,
  onToggle,
  onCopy,
  className,
}) => {
  const formatTime = (date: Date | string) => {
    const dateObj = typeof date === 'string' ? new Date(date) : date;
    return new Intl.DateTimeFormat('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
    }).format(dateObj);
  };

  return (
    <div className={cn(
      "flex items-center justify-between text-sm font-medium mb-2",
      className
    )}>
      <div className="flex items-center gap-2">
        {collapsible && (
          <Button
            variant="ghost"
            size="sm"
            onClick={onToggle}
            className="h-4 w-4 p-0"
          >
            {collapsed ? (
              <ChevronRight className="h-3 w-3" />
            ) : (
              <ChevronDown className="h-3 w-3" />
            )}
          </Button>
        )}
        
        <span>{title}</span>
        
        {indicator && (
          <div className="flex items-center">
            {indicator}
          </div>
        )}
      </div>

      <div className="flex items-center gap-2 text-xs text-muted-foreground font-mono">
        {timestamp && (
          <span className="uppercase tracking-wider">[{formatTime(timestamp)}]</span>
        )}
        
        {onCopy && (
          <Button
            variant="ghost"
            size="sm"
            onClick={onCopy}
            className="h-5 w-5 p-0 opacity-0 group-hover:opacity-100 transition-opacity border border-primary/30 hover:border-primary hover:bg-primary/20"
          >
            <Copy className="h-3 w-3" />
          </Button>
        )}
        
        <Button
          variant="ghost"
          size="sm"
          className="h-5 w-5 p-0 opacity-0 group-hover:opacity-100 transition-opacity border border-primary/30 hover:border-primary hover:bg-primary/20"
        >
          <MoreHorizontal className="h-3 w-3" />
        </Button>
      </div>
    </div>
  );
};
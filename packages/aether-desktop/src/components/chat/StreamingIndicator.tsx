import React from 'react';
import { cn } from '@/lib/utils';

interface StreamingIndicatorProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

export const StreamingIndicator: React.FC<StreamingIndicatorProps> = ({
  size = 'sm',
  className,
}) => {
  const sizeClasses = {
    sm: 'w-1 h-1',
    md: 'w-1.5 h-1.5',
    lg: 'w-2 h-2',
  };

  return (
    <div className={cn("flex items-center gap-1", className)}>
      <div className={cn(
        sizeClasses[size],
        "bg-current rounded-full animate-bounce"
      )} />
      <div className={cn(
        sizeClasses[size],
        "bg-current rounded-full animate-bounce delay-100"
      )} />
      <div className={cn(
        sizeClasses[size],
        "bg-current rounded-full animate-bounce delay-200"
      )} />
    </div>
  );
};

interface ToolStateIndicatorProps {
  state: 'pending' | 'running' | 'completed' | 'failed';
  className?: string;
}

export const ToolStateIndicator: React.FC<ToolStateIndicatorProps> = ({
  state,
  className,
}) => {
  const stateConfig = {
    pending: { color: 'text-gray-500', icon: '⏳' },
    running: { color: 'text-blue-500', icon: <StreamingIndicator size="sm" /> },
    completed: { color: 'text-green-500', icon: '✓' },
    failed: { color: 'text-red-500', icon: '✗' },
  };

  const config = stateConfig[state];

  return (
    <div className={cn("flex items-center gap-1", config.color, className)}>
      {typeof config.icon === 'string' ? (
        <span className="text-xs">{config.icon}</span>
      ) : (
        config.icon
      )}
      <span className="text-xs capitalize">{state}</span>
    </div>
  );
};
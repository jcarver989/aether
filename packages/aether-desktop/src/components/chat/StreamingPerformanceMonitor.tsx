import React, { memo } from 'react';
import { useStreamingMetrics } from '@/hooks/useRAFStreamingOptimization';
import { cn } from '@/lib/utils';

interface StreamingPerformanceMonitorProps {
  className?: string;
  isVisible?: boolean;
}

export const StreamingPerformanceMonitor: React.FC<StreamingPerformanceMonitorProps> = memo(({
  className,
  isVisible = false,
}) => {
  const { metrics } = useStreamingMetrics();

  if (!isVisible) {
    return null;
  }

  return (
    <div className={cn(
      "fixed top-4 right-4 bg-background/90 backdrop-blur-sm border border-border rounded-lg p-3 text-xs font-mono z-50",
      "shadow-lg",
      className
    )}>
      <div className="space-y-1">
        <div className="text-muted-foreground font-semibold mb-2">Performance Metrics</div>
        <div className="flex justify-between gap-4">
          <span className="text-muted-foreground">FPS:</span>
          <span className={cn(
            "font-bold",
            metrics.framesPerSecond >= 55 ? "text-green-500" :
            metrics.framesPerSecond >= 30 ? "text-yellow-500" : "text-red-500"
          )}>
            {metrics.framesPerSecond}
          </span>
        </div>
        <div className="flex justify-between gap-4">
          <span className="text-muted-foreground">Chunks/sec:</span>
          <span className="text-primary">{metrics.chunksPerSecond}</span>
        </div>
        <div className="flex justify-between gap-4">
          <span className="text-muted-foreground">Last chunk:</span>
          <span className="text-muted-foreground">
            {metrics.lastChunkTime ? `${Date.now() - metrics.lastChunkTime}ms` : 'N/A'}
          </span>
        </div>
      </div>
    </div>
  );
});

StreamingPerformanceMonitor.displayName = 'StreamingPerformanceMonitor';
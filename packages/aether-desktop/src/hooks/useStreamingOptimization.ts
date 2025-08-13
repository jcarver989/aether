import { useRef, useEffect, useState } from 'react';
import { useSelector } from './useSelector';
import type { AppState } from '@/state/store';

/**
 * Hook that throttles streaming message updates to prevent excessive re-renders
 * during fast streaming responses
 */
export function useStreamingMessage() {
  const [throttledStreamingMessage, setThrottledStreamingMessage] = useState<
    AppState['streamingMessage']
  >(null);
  
  const streamingMessage = useSelector(state => state.streamingMessage);
  const throttleTimerRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    if (streamingMessage) {
      // If we're receiving streaming content, throttle updates
      if (throttleTimerRef.current) {
        clearTimeout(throttleTimerRef.current);
      }

      throttleTimerRef.current = setTimeout(() => {
        setThrottledStreamingMessage(streamingMessage);
      }, 100); // Update UI every 100ms max for streaming content
    } else {
      // Immediately update when streaming stops
      if (throttleTimerRef.current) {
        clearTimeout(throttleTimerRef.current);
        throttleTimerRef.current = null;
      }
      setThrottledStreamingMessage(streamingMessage);
    }

    return () => {
      if (throttleTimerRef.current) {
        clearTimeout(throttleTimerRef.current);
      }
    };
  }, [streamingMessage]);

  return throttledStreamingMessage;
}

/**
 * Hook that provides optimized selectors for frequently updating state
 */
export function useOptimizedSelectors() {
  const messages = useSelector(state => state.messages);
  const streamingMessage = useStreamingMessage();
  const autoScroll = useSelector(state => state.scroll.autoScroll);
  
  return {
    messages,
    streamingMessage,
    autoScroll,
  };
}
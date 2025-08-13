import { useRef, useEffect, useState, useCallback } from 'react';
import { useSelector } from './useSelector';
import type { AppState } from '@/state/store';

/**
 * Hook that uses requestAnimationFrame to batch streaming message updates
 * for smooth rendering synchronized with the browser's repaint cycle
 */
export function useRAFStreamingMessage() {
  const [throttledStreamingMessage, setThrottledStreamingMessage] = useState<
    AppState['streamingMessage']
  >(null);
  
  const streamingMessage = useSelector(state => state.streamingMessage);
  const rafIdRef = useRef<number | null>(null);
  const lastUpdateRef = useRef<string>('');

  const updateStreamingMessage = useCallback(() => {
    if (streamingMessage && streamingMessage.partialContent !== lastUpdateRef.current) {
      lastUpdateRef.current = streamingMessage.partialContent;
      setThrottledStreamingMessage(streamingMessage);
    }
    rafIdRef.current = null;
  }, [streamingMessage]);

  useEffect(() => {
    if (streamingMessage) {
      // Only schedule a new update if one isn't already pending
      if (rafIdRef.current === null) {
        rafIdRef.current = requestAnimationFrame(updateStreamingMessage);
      }
    } else {
      // Immediately update when streaming stops
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
      setThrottledStreamingMessage(streamingMessage);
      lastUpdateRef.current = '';
    }

    return () => {
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
      }
    };
  }, [streamingMessage, updateStreamingMessage]);

  return throttledStreamingMessage;
}

/**
 * Hook that provides RAF-optimized selectors for frequently updating state
 */
export function useRAFOptimizedSelectors() {
  const messages = useSelector(state => state.messages);
  const streamingMessage = useRAFStreamingMessage();
  const autoScroll = useSelector(state => state.scroll.autoScroll);
  
  return {
    messages,
    streamingMessage,
    autoScroll,
  };
}

/**
 * Hook for monitoring streaming performance metrics
 */
export function useStreamingMetrics() {
  const [metrics, setMetrics] = useState({
    framesPerSecond: 0,
    chunksPerSecond: 0,
    lastChunkTime: 0,
  });
  
  const frameCountRef = useRef(0);
  const chunkCountRef = useRef(0);
  const lastSecondRef = useRef(Date.now());
  const rafRef = useRef<number | null>(null);

  const updateMetrics = useCallback(() => {
    const now = Date.now();
    frameCountRef.current++;
    
    if (now - lastSecondRef.current >= 1000) {
      setMetrics(prev => ({
        ...prev,
        framesPerSecond: frameCountRef.current,
        chunksPerSecond: chunkCountRef.current,
      }));
      
      frameCountRef.current = 0;
      chunkCountRef.current = 0;
      lastSecondRef.current = now;
    }
    
    rafRef.current = requestAnimationFrame(updateMetrics);
  }, []);

  useEffect(() => {
    rafRef.current = requestAnimationFrame(updateMetrics);
    
    return () => {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
      }
    };
  }, [updateMetrics]);

  const recordChunk = useCallback(() => {
    chunkCountRef.current++;
    setMetrics(prev => ({ ...prev, lastChunkTime: Date.now() }));
  }, []);

  return { metrics, recordChunk };
}
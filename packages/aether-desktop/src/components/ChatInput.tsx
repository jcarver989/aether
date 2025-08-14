import React, { useState, useRef, KeyboardEvent } from 'react';
import { Button } from '@/components/ui/button';
import { Send } from 'lucide-react';
import { useAppContext } from '@/hooks/useAppContext';
import { useSelector } from '@/hooks/useSelector';
import { cn } from '@/lib/utils';
import toast from 'react-hot-toast';

interface ChatInputProps {
  className?: string;
}

export const ChatInput: React.FC<ChatInputProps> = ({ className }) => {
  const [input, setInput] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { actions } = useAppContext();
  
  const streamingMessage = useSelector(state => state.streamingMessage);
  const status = useSelector(state => state.status);
  
  const isStreaming = streamingMessage?.isStreaming || false;
  const isAgentReady = status?.connection_status?.provider?.connected === true;

  const handleSubmit = async () => {
    if (!input.trim() || isStreaming) return;
    
    // Check if agent is ready
    if (!isAgentReady) {
      toast.error('Please configure your LLM provider in settings first.');
      return;
    }
    
    const message = input.trim();
    setInput('');
    
    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
    
    try {
      await actions.sendMessage(message);
    } catch (error) {
      console.error('Failed to send message:', error);
      toast.error('Failed to send message. Please try again.');
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    
    // Auto-resize textarea
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = Math.min(textareaRef.current.scrollHeight, 150) + 'px';
    }
  };

  return (
    <div className={cn("border-t border-border bg-gradient-to-r from-background via-background to-card/20 p-6 relative z-10", className)}>
      <div className="max-w-4xl mx-auto">
        <div className="flex gap-4 items-end">
          <div className="flex-1 relative">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={handleInputChange}
              onKeyDown={handleKeyDown}
              placeholder={isStreaming ? "◦ AI PROCESSING..." : "◦ INPUT QUERY..."}
              disabled={isStreaming}
              className={cn(
                "w-full resize-none border border-border/60 bg-gradient-to-br from-card/50 to-card/30 px-4 py-3 text-sm font-mono",
                "placeholder:text-muted-foreground/80 placeholder:font-light",
                "focus:border-primary/80 disabled:cursor-not-allowed disabled:opacity-50",
                "min-h-[48px] max-h-[150px] transition-all duration-200",
                "hover:border-primary/40 text-foreground backdrop-blur-sm",
                "shadow-hologram"
              )}
              rows={1}
            />
            {/* Futuristic cursor effect */}
            {!isStreaming && input === '' && (
              <div className="absolute right-4 top-3.5 w-1.5 h-4 bg-primary animate-cursor-blink opacity-80"></div>
            )}
            {/* Status indicator */}
            <div className="absolute left-2 top-3.5 w-2 h-2 rounded-full bg-primary/60 animate-pulse-glow"></div>
          </div>
          
          <Button
            onClick={handleSubmit}
            disabled={!input.trim() || isStreaming}
            size="sm"
            className={cn(
              "sci-fi-button h-12 w-12 p-0",
              "disabled:opacity-40 disabled:pointer-events-none",
              "shadow-neon-subtle"
            )}
          >
            <Send className="h-4 w-4" />
          </Button>
        </div>
        
        <div className="mt-4 text-xs text-muted-foreground/60 text-center font-mono font-light tracking-wide">
          ENTER ⟩ EXECUTE ⟨ SHIFT+ENTER ⟩ NEW LINE
        </div>
      </div>
    </div>
  );
};
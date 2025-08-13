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
  const config = useSelector(state => state.config);
  
  const isStreaming = streamingMessage?.isStreaming || false;
  const isAgentReady = status?.connection_status?.provider?.connected === true;
  const hasValidConfig = config && (
    (config.active_provider === 'OpenRouter' && config.openrouter_config.api_key) ||
    (config.active_provider === 'Ollama' && config.ollama_config.base_url)
  );

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
    <div className={cn("border-t border-border/40 bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60 p-6", className)}>
      <div className="max-w-4xl mx-auto">
        <div className="flex gap-3 items-end">
          <div className="flex-1 relative">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={handleInputChange}
              onKeyDown={handleKeyDown}
              placeholder={isStreaming ? "Assistant is responding..." : "Type your message..."}
              disabled={isStreaming}
              className={cn(
                "w-full resize-none rounded-xl border border-input bg-background/50 backdrop-blur-sm px-4 py-3 text-sm",
                "ring-offset-background placeholder:text-muted-foreground/60",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/20 focus-visible:ring-offset-1 focus-visible:border-primary/50",
                "disabled:cursor-not-allowed disabled:opacity-50",
                "min-h-[48px] max-h-[150px] transition-all duration-200",
                "hover:border-border/60 hover:bg-background/70"
              )}
              rows={1}
            />
          </div>
          
          <Button
            onClick={handleSubmit}
            disabled={!input.trim() || isStreaming}
            size="sm"
            className={cn(
              "h-12 w-12 p-0 rounded-xl bg-primary hover:bg-primary/90",
              "transition-all duration-200 shadow-lg hover:shadow-xl hover:shadow-primary/20",
              "disabled:opacity-50 disabled:shadow-none"
            )}
          >
            <Send className="h-4 w-4" />
          </Button>
        </div>
        
        <div className="mt-3 text-xs text-muted-foreground/70 text-center">
          Press Enter to send • Shift+Enter for new line
        </div>
      </div>
    </div>
  );
};
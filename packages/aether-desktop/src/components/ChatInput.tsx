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
    <div className={cn("border-t-3 border-border bg-background/98 p-6 relative z-10", className)}>
      <div className="max-w-4xl mx-auto">
        <div className="flex gap-3 items-end">
          <div className="flex-1 relative">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={handleInputChange}
              onKeyDown={handleKeyDown}
              placeholder={isStreaming ? "> ASSISTANT PROCESSING..." : "> ENTER COMMAND..."}
              disabled={isStreaming}
              className={cn(
                "w-full resize-none border-2 border-border bg-card/90 px-4 py-3 text-sm font-mono",
                "placeholder:text-muted-foreground placeholder:uppercase placeholder:tracking-wider",
                "focus:border-primary focus:shadow-retro disabled:cursor-not-allowed disabled:opacity-50",
                "min-h-[48px] max-h-[150px] transition-all duration-100",
                "hover:border-primary/60 text-foreground"
              )}
              rows={1}
            />
            {/* Terminal cursor effect */}
            {!isStreaming && input === '' && (
              <div className="absolute right-3 top-3 w-2 h-5 bg-primary animate-terminal-blink"></div>
            )}
          </div>
          
          <Button
            onClick={handleSubmit}
            disabled={!input.trim() || isStreaming}
            size="sm"
            className={cn(
              "retro-button h-12 w-12 p-0 border-2 border-primary bg-transparent text-primary",
              "hover:bg-primary hover:text-primary-foreground hover:shadow-retro",
              "disabled:opacity-50 disabled:shadow-none transition-all duration-100",
              "uppercase tracking-wide font-bold"
            )}
          >
            <Send className="h-4 w-4" />
          </Button>
        </div>
        
        <div className="mt-3 text-xs text-muted-foreground uppercase tracking-wider text-center font-mono">
          [ENTER] TO EXECUTE • [SHIFT+ENTER] FOR NEW LINE
        </div>
      </div>
    </div>
  );
};
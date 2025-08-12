# Task 4: Build Block-Based UI Components

## Overview
Create a comprehensive UI component library that renders chat messages as distinct blocks, similar to the TUI implementation. Each message type gets its own styled block with consistent theming and interactive features.

## Goals
- Build reusable block components for each message type
- Implement consistent visual design with rounded borders and padding
- Add collapse/expand functionality for tool calls
- Create streaming indicators and animations
- Use shadcn/ui components for consistent styling
- Ensure accessibility and keyboard navigation

## Steps

### 4.1 Create Block Header Component
**File**: `packages/aether-desktop/src/components/chat/BlockHeader.tsx`

```tsx
import React from 'react';
import { ChevronDown, ChevronRight, Copy, MoreHorizontal } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

interface BlockHeaderProps {
  title: string;
  timestamp?: Date;
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
  const formatTime = (date: Date) => {
    return new Intl.DateTimeFormat('en-US', {
      hour: '2-digit',
      minute: '2-digit',
    }).format(date);
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

      <div className="flex items-center gap-1 text-xs text-muted-foreground">
        {timestamp && (
          <span>{formatTime(timestamp)}</span>
        )}
        
        {onCopy && (
          <Button
            variant="ghost"
            size="sm"
            onClick={onCopy}
            className="h-4 w-4 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
          >
            <Copy className="h-3 w-3" />
          </Button>
        )}
        
        <Button
          variant="ghost"
          size="sm"
          className="h-4 w-4 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
        >
          <MoreHorizontal className="h-3 w-3" />
        </Button>
      </div>
    </div>
  );
};
```

### 4.2 Create Streaming Indicator Component
**File**: `packages/aether-desktop/src/components/chat/StreamingIndicator.tsx`

```tsx
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
```

### 4.3 Create System Message Block
**File**: `packages/aether-desktop/src/components/chat/blocks/SystemBlock.tsx`

```tsx
import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';

interface SystemBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const SystemBlock: React.FC<SystemBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  if (block.message.type !== 'system') {
    throw new Error('SystemBlock can only render system messages');
  }

  return (
    <div className={cn(
      "group rounded-lg border border-gray-300 bg-gray-50 p-4 mb-3",
      "transition-all duration-200 hover:shadow-sm",
      className
    )}>
      <BlockHeader
        title="System"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-gray-700"
      />
      
      <div className="text-sm text-gray-800 whitespace-pre-wrap">
        {block.message.content}
      </div>
    </div>
  );
};
```

### 4.4 Create User Message Block
**File**: `packages/aether-desktop/src/components/chat/blocks/UserBlock.tsx`

```tsx
import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';

interface UserBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const UserBlock: React.FC<UserBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  if (block.message.type !== 'user') {
    throw new Error('UserBlock can only render user messages');
  }

  return (
    <div className={cn(
      "group rounded-lg border border-blue-300 bg-blue-50 p-4 mb-3",
      "transition-all duration-200 hover:shadow-sm",
      className
    )}>
      <BlockHeader
        title="You"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-blue-700"
      />
      
      <div className="text-sm text-blue-900 whitespace-pre-wrap">
        {block.message.content}
      </div>
    </div>
  );
};
```

### 4.5 Create Assistant Message Block
**File**: `packages/aether-desktop/src/components/chat/blocks/AssistantBlock.tsx`

```tsx
import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { MarkdownRenderer } from '../MarkdownRenderer';

interface AssistantBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const AssistantBlock: React.FC<AssistantBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  if (block.message.type !== 'assistant') {
    throw new Error('AssistantBlock can only render assistant messages');
  }

  return (
    <div className={cn(
      "group rounded-lg border border-green-300 bg-green-50 p-4 mb-3",
      "transition-all duration-200 hover:shadow-sm",
      className
    )}>
      <BlockHeader
        title="Assistant"
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className="text-green-700"
      />
      
      <div className="text-sm text-green-900">
        <MarkdownRenderer content={block.message.content} />
      </div>
    </div>
  );
};
```

### 4.6 Create Streaming Assistant Block
**File**: `packages/aether-desktop/src/components/chat/blocks/StreamingAssistantBlock.tsx`

```tsx
import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { StreamingIndicator } from '../StreamingIndicator';
import { StreamingMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { MarkdownRenderer } from '../MarkdownRenderer';

interface StreamingAssistantBlockProps {
  block: StreamingMessageBlock;
  className?: string;
}

export const StreamingAssistantBlock: React.FC<StreamingAssistantBlockProps> = ({
  block,
  className,
}) => {
  return (
    <div className={cn(
      "group rounded-lg border border-green-300 bg-green-50 p-4 mb-3",
      "transition-all duration-200 hover:shadow-sm",
      "animate-pulse-subtle", // Custom animation for streaming
      className
    )}>
      <BlockHeader
        title="Assistant"
        timestamp={block.message.timestamp}
        indicator={<StreamingIndicator />}
        className="text-green-700"
      />
      
      <div className="text-sm text-green-900">
        <MarkdownRenderer content={block.partialContent} />
        <span className="inline-block w-2 h-4 bg-green-600 animate-pulse ml-1" />
      </div>
    </div>
  );
};
```

### 4.7 Create Tool Call Block
**File**: `packages/aether-desktop/src/components/chat/blocks/ToolCallBlock.tsx`

```tsx
import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ToolStateIndicator } from '../StreamingIndicator';
import { ChatMessageBlock, ToolCallState } from '@/types';
import { cn } from '@/lib/utils';
import { useAppContext } from '@/hooks/useAppContext';
import { useSelector } from '@/hooks/useSelector';

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
  
  const toolState = useSelector(state => {
    if (block.message.type === 'tool_call') {
      return state.toolCalls.get(block.message.id) || 'pending';
    }
    return 'pending';
  });

  if (block.message.type !== 'tool_call') {
    throw new Error('ToolCallBlock can only render tool_call messages');
  }

  const handleToggle = () => {
    if (onToggle) {
      onToggle();
    } else {
      actions.chat.toggleMessageCollapse(block.id);
    }
  };

  const handleExecute = async () => {
    try {
      await actions.chat.executeToolCall(
        block.message.id,
        block.message.name,
        block.message.params
      );
    } catch (error) {
      console.error('Failed to execute tool:', error);
    }
  };

  return (
    <div className={cn(
      "group rounded-lg border border-purple-300 bg-purple-50 p-4 mb-3",
      "transition-all duration-200 hover:shadow-sm",
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
        className="text-purple-700"
      />
      
      {!block.collapsed && (
        <div className="mt-3">
          <div className="text-xs text-purple-600 mb-2">Parameters:</div>
          <pre className="text-xs bg-purple-100 rounded p-2 overflow-x-auto">
            {JSON.stringify(block.message.params, null, 2)}
          </pre>
          
          {toolState === 'pending' && (
            <button
              onClick={handleExecute}
              className="mt-2 text-xs bg-purple-600 text-white px-2 py-1 rounded hover:bg-purple-700 transition-colors"
            >
              Execute Tool
            </button>
          )}
        </div>
      )}
    </div>
  );
};
```

### 4.8 Create Tool Result Block
**File**: `packages/aether-desktop/src/components/chat/blocks/ToolResultBlock.tsx`

```tsx
import React, { useState } from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { ChevronDown, ChevronUp } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface ToolResultBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const ToolResultBlock: React.FC<ToolResultBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  const [expanded, setExpanded] = useState(false);
  
  if (block.message.type !== 'tool_result') {
    throw new Error('ToolResultBlock can only render tool_result messages');
  }

  const isLongContent = block.message.content.length > 500;
  const displayContent = expanded || !isLongContent 
    ? block.message.content 
    : block.message.content.slice(0, 500) + '...';

  const success = block.message.success !== false; // Default to true if not specified
  const borderColor = success ? 'border-orange-300' : 'border-red-300';
  const bgColor = success ? 'bg-orange-50' : 'bg-red-50';
  const textColor = success ? 'text-orange-700' : 'text-red-700';
  const contentColor = success ? 'text-orange-900' : 'text-red-900';

  return (
    <div className={cn(
      "group rounded-lg border p-4 mb-3",
      "transition-all duration-200 hover:shadow-sm",
      borderColor,
      bgColor,
      className
    )}>
      <BlockHeader
        title={`Tool Result ${success ? '✓' : '✗'}`}
        timestamp={block.message.timestamp}
        onCopy={onCopy}
        className={textColor}
      />
      
      <div className={cn("text-sm whitespace-pre-wrap", contentColor)}>
        {displayContent}
      </div>
      
      {isLongContent && (
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setExpanded(!expanded)}
          className="mt-2 h-auto p-1 text-xs"
        >
          {expanded ? (
            <>
              <ChevronUp className="w-3 h-3 mr-1" />
              Show Less
            </>
          ) : (
            <>
              <ChevronDown className="w-3 h-3 mr-1" />
              Show More
            </>
          )}
        </Button>
      )}
    </div>
  );
};
```

### 4.9 Create Error Message Block
**File**: `packages/aether-desktop/src/components/chat/blocks/ErrorBlock.tsx`

```tsx
import React from 'react';
import { BlockHeader } from '../BlockHeader';
import { ChatMessageBlock } from '@/types';
import { cn } from '@/lib/utils';
import { AlertCircle } from 'lucide-react';

interface ErrorBlockProps {
  block: ChatMessageBlock;
  onCopy?: () => void;
  className?: string;
}

export const ErrorBlock: React.FC<ErrorBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  if (block.message.type !== 'error') {
    throw new Error('ErrorBlock can only render error messages');
  }

  const sourceLabel = block.message.source ? ` (${block.message.source})` : '';

  return (
    <div className={cn(
      "group rounded-lg border border-red-300 bg-red-50 p-4 mb-3",
      "transition-all duration-200 hover:shadow-sm",
      className
    )}>
      <BlockHeader
        title={`Error${sourceLabel}`}
        timestamp={block.message.timestamp}
        indicator={<AlertCircle className="w-4 h-4 text-red-500" />}
        onCopy={onCopy}
        className="text-red-700"
      />
      
      <div className="text-sm text-red-900 font-mono">
        {block.message.message}
      </div>
    </div>
  );
};
```

### 4.10 Create Message Block Router
**File**: `packages/aether-desktop/src/components/chat/MessageBlock.tsx`

```tsx
import React from 'react';
import { ChatMessageBlock, StreamingMessageBlock } from '@/types';
import { SystemBlock } from './blocks/SystemBlock';
import { UserBlock } from './blocks/UserBlock';
import { AssistantBlock } from './blocks/AssistantBlock';
import { StreamingAssistantBlock } from './blocks/StreamingAssistantBlock';
import { ToolCallBlock } from './blocks/ToolCallBlock';
import { ToolResultBlock } from './blocks/ToolResultBlock';
import { ErrorBlock } from './blocks/ErrorBlock';

interface MessageBlockProps {
  block: ChatMessageBlock | StreamingMessageBlock;
  onCopy?: (content: string) => void;
  className?: string;
}

export const MessageBlock: React.FC<MessageBlockProps> = ({
  block,
  onCopy,
  className,
}) => {
  const handleCopy = () => {
    if (onCopy) {
      const content = 'partialContent' in block 
        ? block.partialContent 
        : getMessageContent(block.message);
      onCopy(content);
    }
  };

  // Handle streaming messages
  if ('isStreaming' in block && block.isStreaming) {
    return (
      <StreamingAssistantBlock 
        block={block as StreamingMessageBlock}
        className={className}
      />
    );
  }

  // Handle regular messages
  const regularBlock = block as ChatMessageBlock;
  
  switch (regularBlock.message.type) {
    case 'system':
      return (
        <SystemBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    case 'user':
      return (
        <UserBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    case 'assistant':
      return (
        <AssistantBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    case 'tool_call':
      return (
        <ToolCallBlock 
          block={regularBlock}
          className={className}
        />
      );
      
    case 'tool_result':
      return (
        <ToolResultBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    case 'error':
      return (
        <ErrorBlock 
          block={regularBlock}
          onCopy={handleCopy}
          className={className}
        />
      );
      
    default:
      return (
        <div className="rounded-lg border border-gray-300 bg-gray-50 p-4 mb-3">
          <div className="text-sm text-gray-500">
            Unknown message type: {(regularBlock.message as any).type}
          </div>
        </div>
      );
  }
};

function getMessageContent(message: any): string {
  switch (message.type) {
    case 'system':
    case 'user':
    case 'assistant':
      return message.content;
    case 'tool_call':
      return JSON.stringify(message.params, null, 2);
    case 'tool_result':
      return message.content;
    case 'error':
      return message.message;
    default:
      return '';
  }
}
```

### 4.11 Create Markdown Renderer
**File**: `packages/aether-desktop/src/components/chat/MarkdownRenderer.tsx`

```tsx
import React from 'react';
import { cn } from '@/lib/utils';

interface MarkdownRendererProps {
  content: string;
  className?: string;
}

// Simple markdown renderer for now - can be enhanced later
export const MarkdownRenderer: React.FC<MarkdownRendererProps> = ({
  content,
  className,
}) => {
  // For now, just render as preformatted text
  // TODO: Add proper markdown parsing with react-markdown
  
  return (
    <div className={cn("whitespace-pre-wrap", className)}>
      {content}
    </div>
  );
};

// TODO: Implement proper markdown renderer
// This would use react-markdown with plugins for:
// - Syntax highlighting (prism-react-renderer)
// - Code block copy buttons
// - LaTeX math (optional)
// - Tables, links, etc.
```

### 4.12 Add Custom Animations
**File**: `packages/aether-desktop/tailwind.config.js` (create if not exists)

```javascript
/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      animation: {
        'pulse-subtle': 'pulse-subtle 2s ease-in-out infinite',
      },
      keyframes: {
        'pulse-subtle': {
          '0%, 100%': { 
            opacity: '1',
            transform: 'scale(1)' 
          },
          '50%': { 
            opacity: '0.95',
            transform: 'scale(1.002)' 
          },
        }
      }
    },
  },
  plugins: [],
}
```

## Testing

### 4.13 Create Component Tests
**File**: `packages/aether-desktop/src/components/chat/__tests__/MessageBlock.test.tsx`

```tsx
import React from 'react';
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { MessageBlock } from '../MessageBlock';
import { ChatMessageBlock } from '@/types';

const mockUserMessage: ChatMessageBlock = {
  id: '1',
  message: {
    type: 'user',
    content: 'Hello, world!',
    timestamp: new Date('2023-01-01T12:00:00Z'),
  },
};

const mockAssistantMessage: ChatMessageBlock = {
  id: '2',
  message: {
    type: 'assistant',
    content: 'Hello! How can I help you?',
    timestamp: new Date('2023-01-01T12:01:00Z'),
  },
};

describe('MessageBlock', () => {
  it('renders user messages correctly', () => {
    render(<MessageBlock block={mockUserMessage} />);
    
    expect(screen.getByText('You')).toBeInTheDocument();
    expect(screen.getByText('Hello, world!')).toBeInTheDocument();
  });

  it('renders assistant messages correctly', () => {
    render(<MessageBlock block={mockAssistantMessage} />);
    
    expect(screen.getByText('Assistant')).toBeInTheDocument();
    expect(screen.getByText('Hello! How can I help you?')).toBeInTheDocument();
  });

  it('displays timestamps', () => {
    render(<MessageBlock block={mockUserMessage} />);
    
    // Should display formatted time
    expect(screen.getByText('12:00')).toBeInTheDocument();
  });
});
```

## Acceptance Criteria
- [ ] All message types have dedicated block components
- [ ] Consistent visual design with rounded borders and role-based colors
- [ ] Collapsible tool calls with expand/collapse animation
- [ ] Streaming indicators for real-time content
- [ ] Copy functionality for all message types
- [ ] Proper timestamp formatting
- [ ] Hover effects and interactive elements
- [ ] Responsive design that works on different screen sizes
- [ ] Accessibility features (keyboard navigation, screen reader support)
- [ ] Comprehensive test coverage for all components

## Dependencies
- Task 2: TypeScript Types (for message interfaces)
- Task 3: Action Classes (for interactive functionality)

## Next Steps
After completing this task, proceed to:
- Task 5: Add streaming support with Tauri Channels
- Task 6: Create testing infrastructure with mock channels
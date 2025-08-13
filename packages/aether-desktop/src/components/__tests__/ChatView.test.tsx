import React from 'react';
import { render, screen } from '@testing-library/react';
import { ChatView } from '../ChatView';
import { AppContextProvider } from '../AppContextProvider';

// Mock the ScrollArea component to test the scrolling logic
jest.mock('@/components/ui/scroll-area', () => ({
  ScrollArea: React.forwardRef<HTMLDivElement, any>(({ children, onScrollCapture, className }, ref) => (
    <div 
      ref={ref} 
      className={className}
      onScroll={onScrollCapture}
      data-testid="scroll-area"
      style={{ 
        height: '200px', 
        overflowY: 'auto',
        border: '1px solid #ccc' // Make it visible for debugging
      }}
    >
      {children}
    </div>
  ))
}));

// Mock toast
jest.mock('react-hot-toast', () => ({
  success: jest.fn(),
  error: jest.fn(),
}));

const mockActions = {
  enableAutoScroll: jest.fn(),
  disableAutoScroll: jest.fn(),
};

const mockState = {
  messages: [
    {
      id: '1',
      type: 'user' as const,
      content: 'Hello',
      timestamp: new Date().toISOString(),
    },
    {
      id: '2', 
      type: 'assistant' as const,
      content: 'Hi there!',
      timestamp: new Date().toISOString(),
    },
  ],
  streamingMessage: null,
  scroll: {
    autoScroll: true,
  },
};

// Mock the context and hooks
jest.mock('@/hooks/useAppContext', () => ({
  useAppContext: () => ({ actions: mockActions }),
}));

jest.mock('@/hooks/useSelector', () => ({
  useSelector: (selector: any) => selector(mockState),
}));

describe('ChatView Scrolling', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('should render messages in a scrollable area', () => {
    render(<ChatView />);
    
    const scrollArea = screen.getByTestId('scroll-area');
    expect(scrollArea).toBeInTheDocument();
    expect(scrollArea).toHaveStyle({ overflowY: 'auto' });
  });

  it('should have scroll event handler attached', () => {
    render(<ChatView />);
    
    const scrollArea = screen.getByTestId('scroll-area');
    expect(scrollArea).toHaveAttribute('onScroll');
  });

  // Test case that will expose the bug: 
  // The ref points to inner div but scroll events come from ScrollArea
  it('should handle auto-scroll correctly when content overflows', () => {
    const { rerender } = render(<ChatView />);
    
    // Add many messages to cause overflow
    const manyMessages = Array.from({ length: 50 }, (_, i) => ({
      id: `msg-${i}`,
      type: 'user' as const,
      content: `Message ${i} - This is a long message that will cause content to overflow the container height`,
      timestamp: new Date().toISOString(),
    }));

    // Update the mock state with many messages
    const mockStateWithManyMessages = {
      ...mockState,
      messages: manyMessages,
    };

    // Re-mock the useSelector hook for this test
    jest.doMock('@/hooks/useSelector', () => ({
      useSelector: (selector: any) => selector(mockStateWithManyMessages),
    }));

    rerender(<ChatView />);

    const scrollArea = screen.getByTestId('scroll-area');
    
    // The scroll area should have overflow content
    expect(scrollArea.scrollHeight).toBeGreaterThan(scrollArea.clientHeight);
    
    // This test will currently fail because the scrollRef points to the wrong element
    // and auto-scroll won't work properly
  });
});
/// Test to verify that scrolling properly clears old content from the buffer
use color_eyre::Result;
use ratatui::{backend::TestBackend, Terminal};
use aether::{
    action::{Action, ScrollDirection},
    components::{Component, virtual_scroll::{VirtualScroll, VirtualScrollItem}},
};
use ratatui::{buffer::Buffer, layout::Rect};
use tokio::sync::mpsc;

// Test item for virtual scroll
#[derive(Debug, Clone)]
struct TestScrollItem {
    content: String,
    height: u16,
}

impl TestScrollItem {
    fn new(content: &str, height: u16) -> Self {
        Self {
            content: content.to_string(),
            height,
        }
    }
}

impl VirtualScrollItem for TestScrollItem {
    fn height(&self, _width: u16) -> u16 {
        self.height
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::widgets::{Paragraph, Widget};
        use ratatui::text::Text;
        
        let text = Text::from(self.content.as_str());
        let paragraph = Paragraph::new(text);
        paragraph.render(area, buf);
    }
}

#[tokio::test]
async fn test_scrolling_clears_old_content() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut scroll: VirtualScroll<TestScrollItem> = VirtualScroll::new();
    scroll.register_action_handler(tx)?;

    // Add items that will definitely fill more than the viewport
    for i in 0..10 {
        scroll.items_mut().push(TestScrollItem::new(&format!("Item {}", i), 3));
    }

    // Create a small terminal to force scrolling (20 lines high)
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend)?;

    // Render initial state
    terminal.draw(|frame| {
        scroll.draw(frame, frame.area()).unwrap();
    })?;

    let initial_buffer = terminal.backend().buffer().clone();

    // Scroll down several times
    for _ in 0..5 {
        scroll.update(Action::ScrollChat(ScrollDirection::Down))?;
    }

    // Render after scrolling
    terminal.draw(|frame| {
        scroll.draw(frame, frame.area()).unwrap();
    })?;

    let scrolled_buffer = terminal.backend().buffer().clone();

    // Check that the content has actually changed
    let initial_content: String = initial_buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    
    let scrolled_content: String = scrolled_buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();

    // The content should be different after scrolling
    assert_ne!(initial_content, scrolled_content, "Content should change after scrolling");

    // More importantly, check that old content is not overlapping with new content
    // by checking that there are no unexpected duplicate symbols in positions 
    // where new content should have completely replaced old content
    
    let initial_lines: Vec<String> = initial_content
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    
    let scrolled_lines: Vec<String> = scrolled_content
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();

    // Verify that we actually have different content in the first visible line
    // This ensures that scrolling is working and old content is properly cleared
    if let (Some(initial_first), Some(scrolled_first)) = (initial_lines.first(), scrolled_lines.first()) {
        if !initial_first.trim().is_empty() && !scrolled_first.trim().is_empty() {
            // If both lines have content, they should be different after scrolling
            assert_ne!(
                initial_first.trim(),
                scrolled_first.trim(),
                "First line should change after scrolling down, indicating old content was cleared"
            );
        }
    }

    Ok(())
}

#[tokio::test] 
async fn test_buffer_clearing_prevents_overlap() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut scroll: VirtualScroll<TestScrollItem> = VirtualScroll::new();
    scroll.register_action_handler(tx)?;

    // Add items with distinct patterns to detect overlap
    scroll.items_mut().push(TestScrollItem::new("AAAAAAAAAA", 1));
    scroll.items_mut().push(TestScrollItem::new("BBBBBBBBBB", 1));
    scroll.items_mut().push(TestScrollItem::new("CCCCCCCCCC", 1));
    scroll.items_mut().push(TestScrollItem::new("DDDDDDDDDD", 1));
    scroll.items_mut().push(TestScrollItem::new("EEEEEEEEEE", 1));

    let backend = TestBackend::new(80, 2);  // Very small viewport
    let mut terminal = Terminal::new(backend)?;

    // Render initial state (should show A and B)
    terminal.draw(|frame| {
        scroll.draw(frame, frame.area()).unwrap();
    })?;

    let initial_buffer = terminal.backend().buffer().clone();
    let initial_content: String = initial_buffer.content.iter().map(|cell| cell.symbol()).collect();

    // Should contain A's and B's, but not C's, D's, or E's
    assert!(initial_content.contains('A'), "Should show first item");
    assert!(!initial_content.contains('E'), "Should not show items far down");

    // Scroll down to show different content
    scroll.update(Action::ScrollChat(ScrollDirection::Down))?;
    scroll.update(Action::ScrollChat(ScrollDirection::Down))?;

    terminal.draw(|frame| {
        scroll.draw(frame, frame.area()).unwrap();
    })?;

    let scrolled_buffer = terminal.backend().buffer().clone();
    let scrolled_content: String = scrolled_buffer.content.iter().map(|cell| cell.symbol()).collect();

    // After scrolling, we should see different content (no A's) and no overlap
    assert!(!scrolled_content.contains('A'), "Old content (A's) should be cleared");
    assert!(scrolled_content.contains('C'), "Should show new content after scrolling");

    // Verify the content is actually different (no overlap from old rendering)
    assert_ne!(initial_content, scrolled_content, "Content should be completely different after scrolling");

    Ok(())
}
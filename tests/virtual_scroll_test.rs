use aether::action::Action;
use aether::components::{
    Component,
    virtual_scroll::{VirtualScroll, VirtualScrollItem},
};
use color_eyre::Result;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::{Paragraph, Widget},
};

#[derive(Debug, Clone, PartialEq)]
struct TestItem {
    text: String,
    height: u16,
}

impl TestItem {
    fn new(text: &str, height: u16) -> Self {
        Self {
            text: text.to_string(),
            height,
        }
    }
}

impl VirtualScrollItem for TestItem {
    fn height(&self, _width: u16) -> u16 {
        self.height
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let paragraph = Paragraph::new(Span::styled(&self.text, Style::default().fg(Color::White)));
        paragraph.render(area, buf);
    }
}

fn create_test_terminal() -> Terminal<TestBackend> {
    let backend = TestBackend::new(80, 24);
    Terminal::new(backend).unwrap()
}

#[test]
fn test_virtual_scroll_renders_visible_items_only() -> Result<()> {
    let mut terminal = create_test_terminal();
    let mut scroll = VirtualScroll::new();

    // Add items with different heights
    scroll.items_mut().push(TestItem::new("Item 1", 2));
    scroll.items_mut().push(TestItem::new("Item 2", 3));
    scroll.items_mut().push(TestItem::new("Item 3", 1));
    scroll.items_mut().push(TestItem::new("Item 4", 4));
    scroll.items_mut().push(TestItem::new("Item 5", 2));

    terminal.draw(|frame| {
        let area = Rect::new(0, 0, 20, 6); // Viewport height 6
        scroll.draw(frame, area).unwrap();
    })?;

    let buffer = terminal.backend().buffer();

    // Should render first few items that fit in viewport height 6
    // Item 1 (height 2) + Item 2 (height 3) + Item 3 (height 1) = 6 total
    assert_eq!(buffer[(0, 0)].symbol(), "I"); // "Item 1"
    assert_eq!(buffer[(0, 2)].symbol(), "I"); // "Item 2" 
    assert_eq!(buffer[(0, 5)].symbol(), "I"); // "Item 3"

    // Item 4 should not be visible (would exceed viewport)
    assert_eq!(buffer[(0, 6)].symbol(), " ");

    Ok(())
}

#[test]
fn test_virtual_scroll_scrolling_updates_visible_items() -> Result<()> {
    let mut terminal = create_test_terminal();
    let mut scroll = VirtualScroll::new();

    // Add many items
    for i in 1..=10 {
        scroll
            .items_mut()
            .push(TestItem::new(&format!("Item {}", i), 1));
    }

    // Initially render
    terminal.draw(|frame| {
        let area = Rect::new(0, 0, 20, 3); // Small viewport
        scroll.draw(frame, area).unwrap();
    })?;

    let buffer_before = terminal.backend().buffer().clone();
    assert_eq!(buffer_before[(0, 0)].symbol(), "I"); // Item 1
    assert_eq!(buffer_before[(0, 1)].symbol(), "I"); // Item 2  
    assert_eq!(buffer_before[(0, 2)].symbol(), "I"); // Item 3

    // Scroll down by 2 lines
    scroll.update(Action::ScrollChat(aether::action::ScrollDirection::Down))?;
    scroll.update(Action::ScrollChat(aether::action::ScrollDirection::Down))?;

    terminal.draw(|frame| {
        let area = Rect::new(0, 0, 20, 3);
        scroll.draw(frame, area).unwrap();
    })?;

    let buffer_after = terminal.backend().buffer();

    // Should now show items 3, 4, 5 (shifted by 2)
    assert_eq!(buffer_after[(0, 0)].symbol(), "I"); // Item 3
    assert_eq!(buffer_after[(0, 1)].symbol(), "I"); // Item 4
    assert_eq!(buffer_after[(0, 2)].symbol(), "I"); // Item 5

    Ok(())
}

#[test]
fn test_virtual_scroll_handles_large_item_counts() -> Result<()> {
    let mut terminal = create_test_terminal();
    let mut scroll = VirtualScroll::new();

    // Add 1000 items to test performance
    for i in 1..=1000 {
        scroll
            .items_mut()
            .push(TestItem::new(&format!("Item {}", i), 1));
    }

    // This should not hang or be slow
    terminal.draw(|frame| {
        let area = Rect::new(0, 0, 20, 5);
        scroll.draw(frame, area).unwrap();
    })?;

    let buffer = terminal.backend().buffer();

    // Should render only first 5 items
    assert_eq!(buffer[(0, 0)].symbol(), "I"); // Item 1
    assert_eq!(buffer[(0, 4)].symbol(), "I"); // Item 5

    // Beyond viewport should be empty
    assert_eq!(buffer[(0, 5)].symbol(), " ");

    Ok(())
}

#[test]
fn test_virtual_scroll_variable_heights() -> Result<()> {
    let mut terminal = create_test_terminal();
    let mut scroll = VirtualScroll::new();

    // Add items with varying heights
    scroll.items_mut().push(TestItem::new("Small", 1)); // 0-1
    scroll.items_mut().push(TestItem::new("Medium", 3)); // 1-4  
    scroll.items_mut().push(TestItem::new("Large", 5)); // 4-9
    scroll.items_mut().push(TestItem::new("Tiny", 1)); // 9-10

    terminal.draw(|frame| {
        let area = Rect::new(0, 0, 20, 6); // Viewport shows items at positions 0-6
        scroll.draw(frame, area).unwrap();
    })?;

    let buffer = terminal.backend().buffer();

    // Should render Small (pos 0) + Medium (pos 1-3) + part of Large (pos 4-5)
    assert_eq!(buffer[(0, 0)].symbol(), "S"); // "Small"
    assert_eq!(buffer[(0, 1)].symbol(), "M"); // "Medium"
    assert_eq!(buffer[(0, 4)].symbol(), "L"); // "Large" (partial)

    // Position 6+ should not show "Tiny" since Large takes up space
    assert_ne!(buffer[(0, 5)].symbol(), "T");

    Ok(())
}

#[test]
fn test_virtual_scroll_empty_list() -> Result<()> {
    let mut terminal = create_test_terminal();
    let mut scroll: VirtualScroll<TestItem> = VirtualScroll::new();

    terminal.draw(|frame| {
        let area = Rect::new(0, 0, 20, 10);
        scroll.draw(frame, area).unwrap();
    })?;

    let buffer = terminal.backend().buffer();

    // Should render nothing
    for y in 0..10 {
        assert_eq!(buffer[(0, y)].symbol(), " ");
    }

    Ok(())
}

/// Test to reproduce and verify fix for scrolling issue
use aether::{
    action::{Action, ScrollDirection},
    components::{Component, home::Home},
    types::ChatMessage,
};
use color_eyre::Result;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use tokio::sync::mpsc;

#[tokio::test]
async fn test_arrow_key_scrolling_with_home_component() -> Result<()> {
    let mut home = Home::new();
    let (tx, _rx) = mpsc::unbounded_channel();

    // Register action handler
    home.register_action_handler(tx.clone())?;

    // Add some messages to make scrolling possible
    let messages = vec![
        ChatMessage::User {
            content: "First message".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::Assistant {
            content: "First response".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::User {
            content: "Second message".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::Assistant {
            content: "Second response".to_string(),
            timestamp: chrono::Utc::now(),
        },
    ];

    // Add messages to the home component
    for message in messages {
        home.update(Action::AddChatMessage(message))?;
    }

    // Now test scrolling actions directly (simulating app.rs behavior)

    // Test ScrollChat Up action
    let result = home.update(Action::ScrollChat(ScrollDirection::Up))?;
    assert_eq!(result, None); // Home component should handle this internally

    // Test ScrollChat Down action
    let result = home.update(Action::ScrollChat(ScrollDirection::Down))?;
    assert_eq!(result, None); // Home component should handle this internally

    // Test PageUp action
    let result = home.update(Action::ScrollChat(ScrollDirection::PageUp))?;
    assert_eq!(result, None); // Home component should handle this internally

    // Test PageDown action
    let result = home.update(Action::ScrollChat(ScrollDirection::PageDown))?;
    assert_eq!(result, None); // Home component should handle this internally

    // Verify the component can render without issues after scrolling
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| {
        home.draw(frame, frame.area()).unwrap();
    })?;

    Ok(())
}

#[tokio::test]
async fn test_virtual_scroll_handles_scroll_actions() -> Result<()> {
    use aether::components::virtual_scroll::{VirtualScroll, VirtualScrollItem};

    // Create a simple test item for virtual scroll
    #[derive(Clone)]
    struct TestItem {
        content: String,
        height: u16,
    }

    impl VirtualScrollItem for TestItem {
        fn height(&self, _width: u16) -> u16 {
            self.height
        }

        fn render(&self, area: ratatui::layout::Rect, buf: &mut Buffer) {
            use ratatui::widgets::{Paragraph, Widget};
            Paragraph::new(self.content.as_str()).render(area, buf);
        }
    }

    let mut scroll: VirtualScroll<TestItem> = VirtualScroll::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    scroll.register_action_handler(tx)?;

    // Add test items
    scroll.items_mut().push(TestItem {
        content: "Item 1".to_string(),
        height: 2,
    });
    scroll.items_mut().push(TestItem {
        content: "Item 2".to_string(),
        height: 2,
    });
    scroll.items_mut().push(TestItem {
        content: "Item 3".to_string(),
        height: 2,
    });
    scroll.items_mut().push(TestItem {
        content: "Item 4".to_string(),
        height: 2,
    });

    // Test that ScrollChat actions are handled correctly
    let result = scroll.update(Action::ScrollChat(ScrollDirection::Up))?;
    assert_eq!(result, None);

    let result = scroll.update(Action::ScrollChat(ScrollDirection::Down))?;
    assert_eq!(result, None);

    let result = scroll.update(Action::ScrollChat(ScrollDirection::PageUp))?;
    assert_eq!(result, None);

    let result = scroll.update(Action::ScrollChat(ScrollDirection::PageDown))?;
    assert_eq!(result, None);

    // Test that the virtual scroll can render
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| {
        scroll.draw(frame, frame.area()).unwrap();
    })?;

    Ok(())
}

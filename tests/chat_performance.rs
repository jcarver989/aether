use aether::action::Action;
use aether::components::Component;
use aether::components::chat::Chat;
use aether::types::ChatMessage;
use chrono::Utc;
use std::time::Instant;

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_content_blocks_not_rebuilt_unnecessarily() {
        let mut chat = Chat::new();

        // Add some messages
        for i in 0..10 {
            let message = ChatMessage::User {
                content: format!("Test message {i}"),
                timestamp: Utc::now(),
            };
            chat.update(Action::AddChatMessage(message)).unwrap();
        }

        // Get initial content blocks
        let initial_blocks_len = chat.get_content_blocks().len();

        // Perform operations that shouldn't trigger rebuild
        chat.update(Action::Tick).unwrap();
        chat.update(Action::Render).unwrap();

        // Content blocks should remain the same
        assert_eq!(chat.get_content_blocks().len(), initial_blocks_len);
    }

    #[test]
    fn test_large_chat_performance() {
        let mut chat = Chat::new();

        // Add a large number of messages to simulate real usage
        let message_count = 1000;
        for i in 0..message_count {
            let message = ChatMessage::Assistant {
                content: format!(
                    "This is a longer assistant message {i} with more content to simulate real usage patterns. It contains multiple lines and more text to test performance under realistic conditions."
                ),
                timestamp: Utc::now(),
            };
            chat.update(Action::AddChatMessage(message)).unwrap();
        }

        // Measure time for operations that should be fast
        let start = Instant::now();

        // These operations should not trigger expensive rebuilds
        for _ in 0..100 {
            chat.update(Action::Tick).unwrap();
            chat.update(Action::Render).unwrap();
        }

        let duration = start.elapsed();

        // Should complete in reasonable time (adjust threshold as needed)
        assert!(
            duration.as_millis() < 100,
            "Operations took too long: {duration:?}"
        );
    }

    #[test]
    fn test_streaming_performance() {
        let mut chat = Chat::new();

        // Start streaming
        chat.update(Action::StartStreaming).unwrap();

        let start = Instant::now();

        // Stream a lot of content rapidly
        for i in 0..1000 {
            chat.update(Action::StreamContent(format!("Content chunk {i} ")))
                .unwrap();
        }

        let duration = start.elapsed();

        // Streaming should be fast
        assert!(
            duration.as_millis() < 500,
            "Streaming took too long: {duration:?}"
        );

        // Complete streaming
        chat.update(Action::StreamComplete).unwrap();
    }

    #[test]
    fn test_content_dirty_flag_behavior() {
        let mut chat = Chat::new();

        // Add a message - should set content_dirty
        let message = ChatMessage::User {
            content: "Test message".to_string(),
            timestamp: Utc::now(),
        };
        chat.update(Action::AddChatMessage(message)).unwrap();

        // After adding message, content should be dirty
        // We'll need to add a method to check this or refactor to make it testable

        // Operations that shouldn't make content dirty
        chat.update(Action::Tick).unwrap();
        chat.update(Action::Render).unwrap();

        // These shouldn't trigger rebuilds
    }
}

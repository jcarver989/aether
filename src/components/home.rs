use color_eyre::Result;
use ratatui::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, chat_virtual::ChatVirtual, input::Input};
use crate::{action::Action, config::Config};

#[derive(Default)]
pub struct Home {
    command_tx: Option<UnboundedSender<Action>>,
    config: Arc<Config>,
    chat: ChatVirtual,
    input: Input,
}

impl Home {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            config: Arc::new(Config::default()),
            chat: ChatVirtual::new(),
            input: Input::new(),
        }
    }
}

impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx.clone());
        self.chat.register_action_handler(tx.clone())?;
        self.input.register_action_handler(tx)?;
        Ok(())
    }

    fn register_config_handler(&mut self, config: Arc<Config>) -> Result<()> {
        self.config = Arc::clone(&config);
        self.chat.register_config_handler(Arc::clone(&config))?;
        self.input.register_config_handler(config)?;
        Ok(())
    }


    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        // Forward actions to child components
        if let Some(child_action) = self.chat.update(action.clone())? {
            return Ok(Some(child_action));
        }
        if let Some(child_action) = self.input.update(action.clone())? {
            return Ok(Some(child_action));
        }

        match action {
            Action::Tick => {
                // add any logic here that should run on every tick
            }
            Action::Render => {
                // add any logic here that should run on every render
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Create vertical layout: chat area (main) and input area (bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Chat area
                Constraint::Length(5), // Input area (increased height for multi-line)
            ])
            .split(area);

        // Render chat and input components
        self.chat.draw(frame, chunks[0])?;
        self.input.draw(frame, chunks[1])?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, CursorDirection};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
    use tokio::sync::mpsc;

    // Test buffer dimensions
    const TEST_BUFFER_WIDTH: u16 = 80;
    const TEST_BUFFER_HEIGHT: u16 = 24;

    /// Helper function to draw home component and return buffer
    fn draw_home_component(home: &mut Home) -> Buffer {
        let backend = TestBackend::new(TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                home.draw(frame, area).expect("Failed to draw home component");
            })
            .expect("Failed to draw terminal frame");

        terminal.backend().buffer().clone()
    }

    /// Helper function to extract text content from buffer
    fn extract_buffer_text(buffer: &Buffer) -> String {
        buffer.content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    #[test]
    fn test_action_processing_no_double_handling() {
        let mut home = Home::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        
        // Register action handler
        home.register_action_handler(tx).expect("Failed to register action handler");
        
        // Draw initial state - should show placeholder
        let initial_buffer = draw_home_component(&mut home);
        let initial_content = extract_buffer_text(&initial_buffer);
        assert!(initial_content.contains("Type your message..."), "Should show placeholder initially");
        
        // Process InsertChar action directly (as app.rs would do)
        let child_action = home.update(Action::InsertChar('h')).expect("Failed to update home with action");
        assert_eq!(child_action, None); // Home doesn't return additional actions for this
        
        // Draw after processing action - should now show the character
        let after_action_buffer = draw_home_component(&mut home);
        let after_action_content = extract_buffer_text(&after_action_buffer);
        assert!(after_action_content.contains("> h"), "Should show 'h' after action processing");
        assert!(!after_action_content.contains("Type your message..."), "Should not show placeholder after typing");
    }
    
    #[test]
    fn test_insert_char_q_action_processing() {
        let mut home = Home::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        
        // Register action handler
        home.register_action_handler(tx).expect("Failed to register action handler");
        
        // Process InsertChar('q') action directly (as app.rs would do)
        let child_action = home.update(Action::InsertChar('q')).expect("Failed to update home with action");
        assert_eq!(child_action, None);
        
        // Draw and verify 'q' appears in the input, proving it was treated as text input
        let buffer = draw_home_component(&mut home);
        let content = extract_buffer_text(&buffer);
        assert!(content.contains("> q"), "Should show 'q' in input field");
        assert!(!content.contains("Type your message..."), "Should not show placeholder after typing");
    }
    
    #[test]
    fn test_scroll_action_processing() {
        let mut home = Home::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        
        // Register action handler
        home.register_action_handler(tx).expect("Failed to register action handler");
        
        // Test that ScrollChat action is processed correctly (now sent directly by app.rs)
        let _result = home.update(Action::ScrollChat(crate::action::ScrollDirection::Up)).expect("Failed to update with ScrollChat");
        
        // Home forwards the action to child components
        // The exact behavior depends on chat implementation, but it should not cause issues
        
        // Draw and verify input field is still showing placeholder (unchanged by scroll action)  
        let buffer = draw_home_component(&mut home);
        let content = extract_buffer_text(&buffer);
        assert!(content.contains("Type your message..."), "Should still show placeholder after scroll action");
    }
    
    #[test]
    fn test_submit_message_action_clears_input() {
        let mut home = Home::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        
        // Register action handler
        home.register_action_handler(tx).expect("Failed to register action handler");
        
        // Add some text first
        home.update(Action::InsertChar('h')).expect("Failed to insert 'h'");
        home.update(Action::InsertChar('i')).expect("Failed to insert 'i'");
        
        // Draw and verify text is visible
        let before_submit_buffer = draw_home_component(&mut home);
        let before_submit_content = extract_buffer_text(&before_submit_buffer);
        assert!(before_submit_content.contains("> hi"), "Should show 'hi' before submit");
        assert!(before_submit_content.contains("1 lines"), "Should show line count hint for single line");
        
        // Test TrySubmitMessage action (simulating Enter key press in app.rs)
        let _child_action = home.update(Action::TrySubmitMessage).expect("Failed to update with TrySubmitMessage");
        // Input component should return SubmitMessage action when not empty
        // This gets processed by the app layer
        
        // Process the SubmitMessage action
        let child_action = home.update(Action::SubmitMessage("hi".to_string())).expect("Failed to update with SubmitMessage");
        assert_eq!(child_action, None);
        
        // Draw and verify input is cleared after submit
        let after_submit_buffer = draw_home_component(&mut home);
        let after_submit_content = extract_buffer_text(&after_submit_buffer);
        assert!(after_submit_content.contains("Type your message..."), "Should show placeholder after submit");
        assert!(!after_submit_content.contains("> hi"), "Should not show 'hi' after submit");
    }
    
    #[test]
    fn test_cursor_movement_action_processing() {
        let mut home = Home::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        
        // Register action handler
        home.register_action_handler(tx).expect("Failed to register action handler");
        
        // Add some text first
        home.update(Action::InsertChar('h')).expect("Failed to insert 'h'");
        home.update(Action::InsertChar('i')).expect("Failed to insert 'i'");
        
        // Draw initial state with cursor at end
        let initial_buffer = draw_home_component(&mut home);
        let initial_content = extract_buffer_text(&initial_buffer);
        assert!(initial_content.contains("> hi"), "Should show 'hi' initially");
        
        // Process MoveCursor action directly (as app.rs would do)
        home.update(Action::MoveCursor(CursorDirection::Left)).expect("Failed to update with MoveCursor");
        
        // Draw after cursor movement - text should still be there but cursor moved
        let after_move_buffer = draw_home_component(&mut home);
        let after_move_content = extract_buffer_text(&after_move_buffer);
        assert!(after_move_content.contains("> hi"), "Should still show 'hi' after cursor move");
        // Note: Cursor position is shown visually in the terminal but hard to test precisely in buffer
        // The important thing is the text is still there and no characters were duplicated
    }
    
    #[test]
    fn test_single_character_insertion() {
        let mut home = Home::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        
        // Register action handler
        home.register_action_handler(tx).expect("Failed to register action handler");
        
        // Process InsertChar action directly (as app.rs would do)
        home.update(Action::InsertChar('x')).expect("Failed to update with action");
        
        // Draw and verify only one 'x' appears
        let buffer = draw_home_component(&mut home);
        let content = extract_buffer_text(&buffer);
        assert!(content.contains("> x"), "Should show single 'x'");
        
        // Count occurrences of 'x' in the visible content
        let x_count = content.matches('x').count();
        assert_eq!(x_count, 1, "Should only have one 'x' in the output, not double characters");
    }
}

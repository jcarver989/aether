use std::time::Instant;

use color_eyre::Result;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Span,
    widgets::Paragraph,
};

use super::Component;

use crate::action::Action;

#[derive(Debug, Clone, PartialEq)]
pub struct FpsCounter {
    last_tick_update: Instant,
    tick_count: u32,
    ticks_per_second: f64,

    last_frame_update: Instant,
    frame_count: u32,
    frames_per_second: f64,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            last_tick_update: Instant::now(),
            tick_count: 0,
            ticks_per_second: 0.0,
            last_frame_update: Instant::now(),
            frame_count: 0,
            frames_per_second: 0.0,
        }
    }

    fn app_tick(&mut self) -> Result<()> {
        self.tick_count += 1;
        let now = Instant::now();
        let elapsed = (now - self.last_tick_update).as_secs_f64();
        if elapsed >= 1.0 {
            self.ticks_per_second = self.tick_count as f64 / elapsed;
            self.last_tick_update = now;
            self.tick_count = 0;
        }
        Ok(())
    }

    fn render_tick(&mut self) -> Result<()> {
        self.frame_count += 1;
        let now = Instant::now();
        let elapsed = (now - self.last_frame_update).as_secs_f64();
        if elapsed >= 1.0 {
            self.frames_per_second = self.frame_count as f64 / elapsed;
            self.last_frame_update = now;
            self.frame_count = 0;
        }
        Ok(())
    }
}

impl Component for FpsCounter {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => self.app_tick()?,
            Action::Render => self.render_tick()?,
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let [top, _] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let message = format!(
            "{:.2} ticks/sec, {:.2} FPS",
            self.ticks_per_second, self.frames_per_second
        );
        let span = Span::styled(message, Style::new().dim());
        let paragraph = Paragraph::new(span).right_aligned();
        frame.render_widget(paragraph, top);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    // Test buffer dimensions - chosen to accommodate the FPS display text
    /// Width that comfortably fits "0.00 ticks/sec, 0.00 FPS" (25 chars) with padding
    const TEST_BUFFER_WIDTH: u16 = 40;
    /// Height that provides space for FPS display plus extra lines to test layout
    const TEST_BUFFER_HEIGHT: u16 = 3;

    /// Helper function to extract text content from a buffer range
    fn extract_buffer_text(buffer: &Buffer, start: usize, end: usize) -> String {
        buffer.content()[start..end]
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    /// Helper function to extract a single line from the buffer
    fn extract_buffer_line(buffer: &Buffer, line: usize, width: usize) -> String {
        let start = line * width;
        let end = start + width;
        extract_buffer_text(buffer, start, end)
    }

    /// Helper function to create terminal and draw FPS counter
    fn draw_fps_counter(fps_counter: &mut FpsCounter, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                fps_counter
                    .draw(frame, area)
                    .expect("Failed to draw FPS counter");
            })
            .expect("Failed to draw terminal frame");

        terminal.backend().buffer().clone()
    }

    #[test]
    fn test_fps_counter_new() {
        let fps_counter = FpsCounter::new();
        assert_eq!(fps_counter.tick_count, 0);
        assert_eq!(fps_counter.ticks_per_second, 0.0);
        assert_eq!(fps_counter.frame_count, 0);
        assert_eq!(fps_counter.frames_per_second, 0.0);
    }

    #[test]
    fn test_fps_counter_default() {
        let fps_counter = FpsCounter::default();
        assert_eq!(fps_counter.tick_count, 0);
        assert_eq!(fps_counter.ticks_per_second, 0.0);
        assert_eq!(fps_counter.frame_count, 0);
        assert_eq!(fps_counter.frames_per_second, 0.0);
    }

    #[test]
    fn test_fps_counter_clone_and_partial_eq() {
        let fps_counter1 = FpsCounter::new();
        let fps_counter2 = fps_counter1.clone();
        assert_eq!(fps_counter1, fps_counter2);
    }

    #[test]
    fn test_update_returns_none_for_all_actions() {
        let mut fps_counter = FpsCounter::new();

        // Test that all actions return None (no further actions emitted)
        let result = fps_counter.update(Action::Tick);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should succeed"), None);

        let result = fps_counter.update(Action::Render);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should succeed"), None);

        let result = fps_counter.update(Action::Quit);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should succeed"), None);
    }

    #[test]
    fn test_draw_renders_correctly() {
        let mut fps_counter = FpsCounter::new();
        let buffer = draw_fps_counter(&mut fps_counter, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Expected content should be right-aligned on the first line
        let expected_content = "0.00 ticks/sec, 0.00 FPS";
        let expected_line = format!(
            "{:>width$}",
            expected_content,
            width = TEST_BUFFER_WIDTH as usize
        );

        let first_line = extract_buffer_line(&buffer, 0, TEST_BUFFER_WIDTH as usize);
        assert_eq!(first_line, expected_line);
    }

    #[test]
    fn test_draw_with_multiple_lines() {
        let mut fps_counter = FpsCounter::new();
        let buffer = draw_fps_counter(&mut fps_counter, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Expected content should be right-aligned on the first line
        let expected_content = "0.00 ticks/sec, 0.00 FPS";
        let expected_line = format!(
            "{:>width$}",
            expected_content,
            width = TEST_BUFFER_WIDTH as usize
        );

        let first_line = extract_buffer_line(&buffer, 0, TEST_BUFFER_WIDTH as usize);
        assert_eq!(first_line, expected_line);

        // Verify remaining lines are empty (layout uses Constraint::Min(0) for remaining space)
        let second_line = extract_buffer_line(&buffer, 1, TEST_BUFFER_WIDTH as usize);
        assert_eq!(second_line, " ".repeat(TEST_BUFFER_WIDTH as usize));

        let third_line = extract_buffer_line(&buffer, 2, TEST_BUFFER_WIDTH as usize);
        assert_eq!(third_line, " ".repeat(TEST_BUFFER_WIDTH as usize));
    }

    #[test]
    fn test_draw_after_actions_shows_zero_rates() {
        let mut fps_counter = FpsCounter::new();

        // Perform several updates but not enough time has passed
        fps_counter
            .update(Action::Tick)
            .expect("Failed to update with Tick");
        fps_counter
            .update(Action::Render)
            .expect("Failed to update with Render");
        fps_counter
            .update(Action::Tick)
            .expect("Failed to update with Tick");
        fps_counter
            .update(Action::Render)
            .expect("Failed to update with Render");

        // Draw and verify the buffer shows 0.00 rates (since not enough time elapsed)
        let buffer = draw_fps_counter(&mut fps_counter, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let expected_content = "0.00 ticks/sec, 0.00 FPS";
        let expected_line = format!(
            "{:>width$}",
            expected_content,
            width = TEST_BUFFER_WIDTH as usize
        );

        let first_line = extract_buffer_line(&buffer, 0, TEST_BUFFER_WIDTH as usize);
        assert_eq!(first_line, expected_line);
    }

    #[test]
    fn test_draw_applies_dim_styling() {
        let mut fps_counter = FpsCounter::new();
        let buffer = draw_fps_counter(&mut fps_counter, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Check that the content cells have the dim modifier applied
        let width = TEST_BUFFER_WIDTH as usize;
        let fps_content_start = width - "0.00 ticks/sec, 0.00 FPS".len();
        for i in fps_content_start..width {
            let cell = &buffer.content()[i];
            if cell.symbol() != " " {
                assert!(cell.modifier.contains(ratatui::style::Modifier::DIM));
            }
        }
    }

    #[test]
    fn test_component_trait_methods_default_implementations() {
        let mut fps_counter = FpsCounter::new();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        // Test register_action_handler (default implementation)
        let result = fps_counter.register_action_handler(tx);
        assert!(result.is_ok());

        // Test register_config_handler (default implementation)
        let config = std::sync::Arc::new(crate::config::Config::default());
        let result = fps_counter.register_config_handler(config);
        assert!(result.is_ok());

        // Test init (default implementation)
        let size = ratatui::layout::Size::new(80, 24);
        let result = fps_counter.init(size);
        assert!(result.is_ok());

        // Test handle_events (default implementation)
        let result = fps_counter.handle_events(None);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should succeed"), None);

        // Test handle_key_event (default implementation)
        let key_event = crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Enter);
        let result = fps_counter.handle_key_event(key_event);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should succeed"), None);

        // Test handle_mouse_event (default implementation)
        let mouse_event = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let result = fps_counter.handle_mouse_event(mouse_event);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should succeed"), None);
    }
}

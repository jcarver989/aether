use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        Self { tick_rate }
    }

    pub fn read_event(&self) -> Result<Option<CrosstermEvent>> {
        if event::poll(self.tick_rate)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    pub fn should_quit(key_event: &KeyEvent) -> bool {
        matches!(
            key_event,
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
        )
    }

    pub fn is_enter(key_event: &KeyEvent) -> bool {
        matches!(key_event, KeyEvent { code: KeyCode::Enter, .. })
    }

    pub fn is_backspace(key_event: &KeyEvent) -> bool {
        matches!(key_event, KeyEvent { code: KeyCode::Backspace, .. })
    }

    pub fn is_char(key_event: &KeyEvent) -> Option<char> {
        match key_event {
            KeyEvent { code: KeyCode::Char(c), .. } => Some(*c),
            _ => None,
        }
    }

    pub fn is_scroll_up(key_event: &KeyEvent) -> bool {
        matches!(key_event, KeyEvent { code: KeyCode::Up, .. })
    }

    pub fn is_scroll_down(key_event: &KeyEvent) -> bool {
        matches!(key_event, KeyEvent { code: KeyCode::Down, .. })
    }

    pub fn is_page_up(key_event: &KeyEvent) -> bool {
        matches!(key_event, KeyEvent { code: KeyCode::PageUp, .. })
    }

    pub fn is_page_down(key_event: &KeyEvent) -> bool {
        matches!(key_event, KeyEvent { code: KeyCode::PageDown, .. })
    }
}
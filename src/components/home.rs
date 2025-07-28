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

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        use crossterm::event::{KeyCode, KeyModifiers};
        
        // Let chat handle scrolling keys first (Up/Down/PageUp/PageDown)
        match (key.code, key.modifiers) {
            (KeyCode::Up, KeyModifiers::NONE) 
            | (KeyCode::Down, KeyModifiers::NONE)
            | (KeyCode::Up, KeyModifiers::CONTROL)
            | (KeyCode::Down, KeyModifiers::CONTROL)
            | (KeyCode::PageUp, _)
            | (KeyCode::PageDown, _) => {
                if let Some(action) = self.chat.handle_key_event(key)? {
                    return Ok(Some(action));
                }
            }
            _ => {
                // For all other keys, let input handle them first
                if let Some(action) = self.input.handle_key_event(key)? {
                    return Ok(Some(action));
                }
                // Then let chat handle any remaining keys
                if let Some(action) = self.chat.handle_key_event(key)? {
                    return Ok(Some(action));
                }
            }
        }
        Ok(None)
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

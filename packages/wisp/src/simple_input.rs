use crate::colors;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::Stylize,
    terminal::{self, ClearType},
};
use std::io::{stdout, Write};

pub struct SimpleInput {
    content: String,
    cursor_pos: usize,
}

#[derive(Debug)]
pub enum InputResult {
    Submit(String),
    Cancel,
    Exit,
}

impl SimpleInput {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_pos: 0,
        }
    }

    pub fn run(&mut self) -> Result<InputResult, Box<dyn std::error::Error>> {
        let mut stdout = stdout();

        // Enable raw mode
        terminal::enable_raw_mode()?;

        // Show initial prompt
        self.show_prompt(&mut stdout)?;
        self.update_display(&mut stdout)?;

        let result = loop {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    match self.handle_key(key_event.code, key_event.modifiers) {
                        Some(result) => break result,
                        None => {
                            self.update_display(&mut stdout)?;
                        }
                    }
                }
                _ => {}
            }
        };

        // Cleanup
        terminal::disable_raw_mode()?;
        execute!(stdout, cursor::Show)?;
        println!(); // Move to next line

        Ok(result)
    }

    fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Option<InputResult> {
        match (key, modifiers) {
            // Submit on Enter
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if !self.content.trim().is_empty() {
                    Some(InputResult::Submit(self.content.clone()))
                } else {
                    None
                }
            }

            // Exit on Ctrl+C
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(InputResult::Exit),

            // Cancel on Escape
            (KeyCode::Esc, KeyModifiers::NONE) => Some(InputResult::Cancel),

            // Clear on Ctrl+L
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                self.content.clear();
                self.cursor_pos = 0;
                None
            }

            // Navigation
            (KeyCode::Left, KeyModifiers::NONE) => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                None
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                if self.cursor_pos < self.content.len() {
                    self.cursor_pos += 1;
                }
                None
            }
            (KeyCode::Home, KeyModifiers::NONE) => {
                self.cursor_pos = 0;
                None
            }
            (KeyCode::End, KeyModifiers::NONE) => {
                self.cursor_pos = self.content.len();
                None
            }

            // Editing
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                if self.cursor_pos > 0 {
                    self.content.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
                None
            }
            (KeyCode::Delete, KeyModifiers::NONE) => {
                if self.cursor_pos < self.content.len() {
                    self.content.remove(self.cursor_pos);
                }
                None
            }

            // Character input
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                self.content.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                None
            }

            _ => None,
        }
    }

    fn show_prompt(&self, stdout: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
        execute!(stdout, cursor::Hide)?;

        queue!(
            stdout,
            crossterm::style::PrintStyledContent(
                "\nEnter message (Enter: send, Ctrl+C: exit, Esc: cancel)\n"
                    .with(colors::info())
                    .dim()
            )
        )?;

        queue!(
            stdout,
            crossterm::style::PrintStyledContent(
                "> ".with(colors::accent()).bold()
            )
        )?;

        stdout.flush()?;
        Ok(())
    }

    fn update_display(&self, stdout: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
        // Clear the current line from the prompt onwards and redraw
        queue!(stdout, cursor::MoveToColumn(0))?;
        queue!(
            stdout,
            crossterm::style::PrintStyledContent(
                "> ".with(colors::accent()).bold()
            )
        )?;
        queue!(stdout, terminal::Clear(ClearType::UntilNewLine))?;

        // Show content with cursor
        let (before_cursor, after_cursor) = self.content.split_at(self.cursor_pos);

        queue!(
            stdout,
            crossterm::style::PrintStyledContent(before_cursor.with(colors::text_primary()))
        )?;
        queue!(
            stdout,
            crossterm::style::PrintStyledContent("█".with(colors::accent()).bold())
        )?;
        queue!(
            stdout,
            crossterm::style::PrintStyledContent(after_cursor.with(colors::text_primary()))
        )?;

        stdout.flush()?;
        Ok(())
    }
}
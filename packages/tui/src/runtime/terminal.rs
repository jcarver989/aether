use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io;

pub fn terminal_size() -> io::Result<(u16, u16)> {
    crossterm::terminal::size()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseCapture {
    Disabled,
    Enabled,
}

pub struct TerminalSession {
    enable_bracketed_paste: bool,
    mouse_capture: MouseCapture,
    cleaned_up: bool,
}

impl TerminalSession {
    pub fn enter(enable_bracketed_paste: bool, mouse_capture: MouseCapture) -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();

        if enable_bracketed_paste {
            crossterm::execute!(stdout, EnableBracketedPaste)?;
        }
        if mouse_capture == MouseCapture::Enabled {
            crossterm::execute!(stdout, EnableMouseCapture)?;
        }

        Ok(Self {
            enable_bracketed_paste,
            mouse_capture,
            cleaned_up: false,
        })
    }

    pub fn cleanup(&mut self) -> io::Result<()> {
        if self.cleaned_up {
            return Ok(());
        }

        let mut stdout = io::stdout();
        if self.mouse_capture == MouseCapture::Enabled {
            crossterm::execute!(stdout, DisableMouseCapture)?;
        }
        if self.enable_bracketed_paste {
            crossterm::execute!(stdout, DisableBracketedPaste)?;
        }
        disable_raw_mode()?;
        self.cleaned_up = true;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

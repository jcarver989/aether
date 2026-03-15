use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::execute;
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
}

impl TerminalSession {
    pub fn new(enable_bracketed_paste: bool, mouse_capture: MouseCapture) -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();

        if enable_bracketed_paste {
            execute!(stdout, EnableBracketedPaste)?;
        }
        if mouse_capture == MouseCapture::Enabled {
            execute!(stdout, EnableMouseCapture)?;
        }

        Ok(Self {
            enable_bracketed_paste,
            mouse_capture,
        })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        if self.mouse_capture == MouseCapture::Enabled {
            let _ = execute!(stdout, DisableMouseCapture);
        }
        if self.enable_bracketed_paste {
            let _ = execute!(stdout, DisableBracketedPaste);
        }
        let _ = disable_raw_mode();
    }
}

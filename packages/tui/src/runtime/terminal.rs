use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io;

pub fn terminal_size() -> io::Result<(u16, u16)> {
    crossterm::terminal::size()
}

pub struct TerminalSession {
    enable_bracketed_paste: bool,
    cleaned_up: bool,
}

impl TerminalSession {
    pub fn enter(enable_bracketed_paste: bool) -> io::Result<Self> {
        enable_raw_mode()?;
        if enable_bracketed_paste {
            crossterm::execute!(io::stdout(), crossterm::event::EnableBracketedPaste)?;
        }

        Ok(Self {
            enable_bracketed_paste,
            cleaned_up: false,
        })
    }

    pub fn cleanup(&mut self) -> io::Result<()> {
        if self.cleaned_up {
            return Ok(());
        }

        if self.enable_bracketed_paste {
            crossterm::execute!(io::stdout(), crossterm::event::DisableBracketedPaste)?;
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

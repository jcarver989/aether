use crossterm::event::Event as CrosstermEvent;
use std::io::{self, Write};
use std::process::{Command, ExitStatus};

use crate::rendering::frame::Frame;
use crate::rendering::render_context::ViewContext;
use crate::rendering::renderer::{Renderer, RendererCommand};
use crate::theme::Theme;

use super::terminal::{MouseCapture, TerminalSession};
use super::{EventTaskHandle, spawn_terminal_event_task};

pub struct TerminalConfig {
    pub bracketed_paste: bool,
    pub mouse_capture: MouseCapture,
}

pub struct TerminalRuntime<T: Write> {
    renderer: Renderer<T>,
    session: Option<TerminalSession>,
    event_task: Option<EventTaskHandle>,
    config: TerminalConfig,
}

impl<T: Write> TerminalRuntime<T> {
    pub fn new(writer: T, theme: Theme, size: (u16, u16), config: TerminalConfig) -> io::Result<Self> {
        let renderer = Renderer::new(writer, theme, size);
        let session = Some(TerminalSession::new(config.bracketed_paste, config.mouse_capture)?);
        let event_task = Some(spawn_terminal_event_task());
        Ok(Self { renderer, session, event_task, config })
    }

    pub async fn next_event(&mut self) -> Option<CrosstermEvent> {
        let task = self.event_task.as_mut()?;
        task.rx().recv().await
    }

    pub fn render_frame(&mut self, f: impl FnOnce(&ViewContext) -> Frame) -> io::Result<()> {
        self.renderer.render_frame(f)
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        self.renderer.clear_screen()
    }

    pub fn on_resize(&mut self, size: (u16, u16)) {
        self.renderer.on_resize(size);
    }

    pub fn apply_commands(&mut self, commands: Vec<RendererCommand>) -> io::Result<()> {
        self.renderer.apply_commands(commands)
    }

    pub async fn suspend(&mut self) -> io::Result<SuspendedTerminal<'_, T>> {
        if let Some(handle) = self.event_task.take() {
            handle.stop().await;
        }
        drop(self.session.take());
        Ok(SuspendedTerminal { runtime: self, resumed: false })
    }

    pub async fn run_external(&mut self, mut command: Command) -> io::Result<ExitStatus> {
        let mut suspended = self.suspend().await?;
        let status = command.status()?;
        suspended.resume()?;
        Ok(status)
    }
}

pub struct SuspendedTerminal<'a, T: Write> {
    runtime: &'a mut TerminalRuntime<T>,
    resumed: bool,
}

impl<T: Write> SuspendedTerminal<'_, T> {
    pub fn resume(&mut self) -> io::Result<()> {
        if self.resumed {
            return Ok(());
        }

        self.runtime.session =
            Some(TerminalSession::new(self.runtime.config.bracketed_paste, self.runtime.config.mouse_capture)?);
        self.runtime.event_task = Some(spawn_terminal_event_task());
        self.runtime.renderer.clear_screen()?;
        self.resumed = true;
        Ok(())
    }
}

impl<T: Write> Drop for SuspendedTerminal<'_, T> {
    fn drop(&mut self) {
        if self.resumed {
            return;
        }

        if self.runtime.session.is_none()
            && let Ok(session) =
                TerminalSession::new(self.runtime.config.bracketed_paste, self.runtime.config.mouse_capture)
        {
            self.runtime.session = Some(session);
        }

        if self.runtime.event_task.is_none() {
            self.runtime.event_task = Some(spawn_terminal_event_task());
        }

        let _ = self.runtime.renderer.clear_screen();
        self.resumed = true;
    }
}

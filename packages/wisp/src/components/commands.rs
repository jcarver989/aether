use crossterm::{
    Command, QueueableCommand,
    cursor::{MoveLeft, MoveRight, MoveTo, MoveToColumn, MoveUp, RestorePosition, SavePosition},
    style::{PrintStyledContent, StyledContent},
    terminal::{Clear, ClearType},
};
use std::io::{Result, Write};

/// Wrapper enum for crossterm commands to enable collecting them
#[derive(Clone)]
#[allow(dead_code)]
pub enum TerminalCommand {
    MoveToColumn(u16),
    MoveLeft,
    MoveRight,
    MoveUp(u16),
    MoveTo(u16, u16),
    SavePosition,
    RestorePosition,
    ClearLine,
    PrintStyled(StyledContent<String>),
    Print(String),
    Clear(ClearType),
}

impl Command for TerminalCommand {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        match self {
            TerminalCommand::MoveToColumn(col) => MoveToColumn(*col).write_ansi(f),
            TerminalCommand::MoveLeft => MoveLeft(1).write_ansi(f),
            TerminalCommand::MoveRight => MoveRight(1).write_ansi(f),
            TerminalCommand::MoveUp(n) => MoveUp(*n).write_ansi(f),
            TerminalCommand::MoveTo(col, row) => MoveTo(*col, *row).write_ansi(f),
            TerminalCommand::SavePosition => SavePosition.write_ansi(f),
            TerminalCommand::RestorePosition => RestorePosition.write_ansi(f),
            TerminalCommand::ClearLine => Clear(ClearType::CurrentLine).write_ansi(f),
            TerminalCommand::PrintStyled(text) => PrintStyledContent(text.clone()).write_ansi(f),
            TerminalCommand::Print(text) => f.write_str(text),
            TerminalCommand::Clear(clear_type) => Clear(*clear_type).write_ansi(f),
        }
    }
}

/// Helper trait to execute a collection of commands
pub trait ExecuteCommands {
    fn flush_commands(&mut self, commands: &[TerminalCommand]) -> Result<()>;
}

impl<T: Write> ExecuteCommands for T {
    fn flush_commands(&mut self, commands: &[TerminalCommand]) -> Result<()> {
        if commands.is_empty() {
            return Ok(());
        }

        for command in commands {
            self.queue(command.clone())?;
        }
        self.flush()
    }
}

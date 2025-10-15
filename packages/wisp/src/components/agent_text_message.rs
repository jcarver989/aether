use crate::output_formatters::ASSISTANT_COLOR;

use super::commands::TerminalCommand;
use crossterm::style::Stylize;

pub struct AgentTextMessage {}

impl AgentTextMessage {
    pub fn render() -> Vec<TerminalCommand> {
        vec![
            TerminalCommand::MoveToColumn(0),
            TerminalCommand::PrintStyled("Wisp:".to_string().with(ASSISTANT_COLOR)),
        ]
    }
}

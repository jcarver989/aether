use crate::output_formatters::ASSISTANT_COLOR;

use super::commands::TerminalCommand;
use crossterm::style::Stylize;

#[allow(dead_code)]
pub struct AgentTextMessage {}

#[allow(dead_code)]
impl AgentTextMessage {
    pub fn render() -> Vec<TerminalCommand> {
        vec![
            TerminalCommand::MoveToColumn(0),
            TerminalCommand::PrintStyled("Wisp:".to_string().with(ASSISTANT_COLOR)),
        ]
    }
}

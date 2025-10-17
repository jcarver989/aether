use super::commands::TerminalCommand;
use crate::render_context::{Component, RenderContext};
use crossterm::style::{Color, Stylize};

pub struct InputPrompt {}

impl Component<()> for InputPrompt {
    fn render(&self, _props: (), _context: &RenderContext) -> Vec<TerminalCommand> {
        let color = Color::Cyan;
        vec![
            TerminalCommand::Print("\r\n".to_string()),
            TerminalCommand::MoveToColumn(0),
            TerminalCommand::PrintStyled("> ".to_string().with(color)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_prompt_implements_component() {
        let input_prompt = InputPrompt {};
        let render_context = RenderContext::new((0, 0), (0, 0));
        let commands = input_prompt.render((), &render_context);

        // Verify we get the expected commands
        assert_eq!(commands.len(), 3);

        // Check first command is a newline
        match &commands[0] {
            TerminalCommand::Print(s) => assert_eq!(s, "\r\n"),
            _ => panic!("Expected Print command"),
        }

        // Check second command is MoveToColumn(0)
        match &commands[1] {
            TerminalCommand::MoveToColumn(col) => assert_eq!(*col, 0),
            _ => panic!("Expected MoveToColumn command"),
        }

        // Check third command is PrintStyled with cyan "> "
        match &commands[2] {
            TerminalCommand::PrintStyled(_) => {
                // We can't easily test the styled content without extracting the style,
                // but we can verify it's the right variant
            }
            _ => panic!("Expected PrintStyled command"),
        }
    }

    #[test]
    fn test_input_prompt_renders_consistently() {
        let input_prompt = InputPrompt {};
        let render_context1 = RenderContext::new((0, 0), (0, 0));
        let render_context2 = RenderContext::new((0, 0), (0, 0));
        let commands1 = input_prompt.render((), &render_context1);
        let commands2 = input_prompt.render((), &render_context2);

        // Verify renders are consistent
        assert_eq!(commands1.len(), commands2.len());
    }
}
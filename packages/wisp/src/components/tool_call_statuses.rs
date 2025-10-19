use crate::components::commands::TerminalCommand;
use crate::render_context::{Component, RenderContext};
use aether::llm::{ToolCallError, ToolCallRequest, ToolCallResult};
use crossterm::style::{StyledContent, Stylize};
use std::collections::HashMap;

const MAX_TOOL_ARG_LENGTH: usize = 200;

/// Props for rendering different tool call status views
pub enum ToolCallStatusViewProps {
    Request(ToolCallRequest),
    Success {
        result: ToolCallResult,
        line_position: u16,
    },
    Error {
        error: ToolCallError,
        line_position: u16,
    },
}

/// View component for rendering tool call statuses
#[derive(Clone)]
pub struct ToolCallStatusView {}

impl Component<ToolCallStatusViewProps> for ToolCallStatusView {
    fn render(
        &self,
        props: ToolCallStatusViewProps,
        context: &RenderContext,
    ) -> Vec<TerminalCommand> {
        match props {
            ToolCallStatusViewProps::Request(request) => {
                let message = format!("● {} running...", request.name).with(context.theme.info);
                let args = Self::format_tool_arguments(&request.arguments, context);

                vec![
                    TerminalCommand::PrintStyled(message),
                    TerminalCommand::PrintStyled(args),
                ]
            }
            ToolCallStatusViewProps::Success {
                result,
                line_position,
            } => {
                let message = format!("● {} ✓", result.name).with(context.theme.success);
                let args = Self::format_tool_arguments(&result.arguments, context);

                vec![
                    TerminalCommand::SavePosition,
                    TerminalCommand::MoveTo(1, line_position),
                    TerminalCommand::ClearLine,
                    TerminalCommand::PrintStyled(message),
                    TerminalCommand::PrintStyled(args),
                    TerminalCommand::Print("\r\n".to_string()),
                    TerminalCommand::RestorePosition,
                ]
            }
            ToolCallStatusViewProps::Error {
                error,
                line_position,
            } => {
                let message = format!("● {} X", error.name).with(context.theme.error);
                let args = error
                    .arguments
                    .as_ref()
                    .map(|a| Self::format_tool_arguments(a, context))
                    .unwrap_or_else(|| "".to_string().with(context.theme.info));

                vec![
                    TerminalCommand::SavePosition,
                    TerminalCommand::MoveTo(1, line_position),
                    TerminalCommand::ClearLine,
                    TerminalCommand::PrintStyled(message),
                    TerminalCommand::PrintStyled(args),
                    TerminalCommand::PrintStyled(error.error.with(context.theme.error)),
                    TerminalCommand::Print("\r\n".to_string()),
                    TerminalCommand::RestorePosition,
                ]
            }
        }
    }
}

impl ToolCallStatusView {
    fn format_tool_arguments(arguments: &str, context: &RenderContext) -> StyledContent<String> {
        let mut formatted = format!(" {arguments}");
        formatted.truncate(MAX_TOOL_ARG_LENGTH);
        formatted.with(context.theme.info)
    }
}

/// Tracks active tool calls and their terminal positions
#[derive(Clone)]
pub struct ToolCallStatuses {
    tool_calls: HashMap<String, ToolCallInfo>,
    view: ToolCallStatusView,
}

#[derive(Clone)]
struct ToolCallInfo {
    line_position: u16,
}

impl ToolCallStatuses {
    pub fn new() -> Self {
        Self {
            tool_calls: HashMap::new(),
            view: ToolCallStatusView {},
        }
    }
}

impl Default for ToolCallStatuses {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallStatuses {
    pub fn on_tool_request(
        &mut self,
        request: &ToolCallRequest,
        context: &RenderContext,
    ) -> Vec<TerminalCommand> {
        if self.tool_calls.contains_key(&request.id) {
            return vec![];
        }

        let (_, current_y) = context.cursor_position;
        let info = ToolCallInfo {
            line_position: current_y,
        };

        self.tool_calls.insert(request.id.to_string(), info);

        let props = ToolCallStatusViewProps::Request(request.clone());
        self.view.render(props, context)
    }

    pub fn on_tool_result(
        &mut self,
        result: &ToolCallResult,
        context: &RenderContext,
    ) -> Vec<TerminalCommand> {
        if let Some(info) = self.tool_calls.remove(&result.id) {
            let props = ToolCallStatusViewProps::Success {
                result: result.clone(),
                line_position: info.line_position,
            };
            self.view.render(props, context)
        } else {
            vec![]
        }
    }

    pub fn on_tool_error(
        &mut self,
        error: &ToolCallError,
        context: &RenderContext,
    ) -> Vec<TerminalCommand> {
        if let Some(info) = self.tool_calls.remove(&error.id) {
            let props = ToolCallStatusViewProps::Error {
                error: error.clone(),
                line_position: info.line_position,
            };
            self.view.render(props, context)
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_context::RenderContext;

    #[test]
    fn test_start_tool_creates_commands() {
        let mut progress_bars = ToolCallStatuses::new();
        let context = RenderContext::new((0, 0), (0, 0));
        let request = ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "/path/to/file".to_string(),
        };
        let commands = progress_bars.on_tool_request(&request, &context);

        assert_eq!(commands.len(), 2);
        assert!(matches!(commands[0], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[1], TerminalCommand::PrintStyled(_)));
    }

    #[test]
    fn test_duplicate_tool_start_returns_empty() {
        let mut progress_bars = ToolCallStatuses::new();
        let context = RenderContext::new((0, 0), (0, 0));
        let request = ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        };
        progress_bars.on_tool_request(&request, &context);
        let commands = progress_bars.on_tool_request(&request, &context);

        assert!(commands.is_empty());
    }

    #[test]
    fn test_update_with_result_uses_saved_position() {
        let mut progress_bars = ToolCallStatuses::new();
        let context = RenderContext::new((0, 0), (0, 0));
        let request = ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        };
        progress_bars.on_tool_request(&request, &context);

        let result = ToolCallResult {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "success".to_string(),
            result: "file contents".to_string(),
        };
        let commands = progress_bars.on_tool_result(&result, &context);

        assert_eq!(commands.len(), 7);
        assert!(matches!(commands[0], TerminalCommand::SavePosition));
        assert!(matches!(commands[1], TerminalCommand::MoveTo(1, _)));
        assert!(matches!(commands[2], TerminalCommand::ClearLine));
        assert!(matches!(commands[3], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[4], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[5], TerminalCommand::Print(ref s) if s == "\r\n"));
        assert!(matches!(commands[6], TerminalCommand::RestorePosition));
    }

    #[test]
    fn test_finish_unknown_tool_returns_empty() {
        let mut progress_bars = ToolCallStatuses::new();
        let context = RenderContext::new((0, 0), (0, 0));
        let result = ToolCallResult {
            id: "unknown".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
            result: "".to_string(),
        };
        let commands = progress_bars.on_tool_result(&result, &context);

        assert!(commands.is_empty());
    }

    #[test]
    fn test_multiple_tools_tracked_independently() {
        let mut progress_bars = ToolCallStatuses::new();
        let context = RenderContext::new((0, 0), (0, 0));
        let request1 = ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        };
        progress_bars.on_tool_request(&request1, &context);
        let first_tool_line = progress_bars
            .tool_calls
            .get("tool-1")
            .unwrap()
            .line_position;

        let request2 = ToolCallRequest {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "".to_string(),
        };
        progress_bars.on_tool_request(&request2, &context);
        let second_tool_line = progress_bars
            .tool_calls
            .get("tool-2")
            .unwrap()
            .line_position;

        let result1 = ToolCallResult {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
            result: "".to_string(),
        };
        let commands1 = progress_bars.on_tool_result(&result1, &context);
        assert!(matches!(commands1[1], TerminalCommand::MoveTo(1, y) if y == first_tool_line));

        let result2 = ToolCallResult {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "".to_string(),
            result: "".to_string(),
        };
        let commands2 = progress_bars.on_tool_result(&result2, &context);
        assert!(matches!(commands2[1], TerminalCommand::MoveTo(1, y) if y == second_tool_line));
    }

    #[test]
    fn test_view_renders_request() {
        let view = ToolCallStatusView {};
        let context = RenderContext::new((0, 0), (0, 0));
        let request = ToolCallRequest {
            id: "test-1".to_string(),
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
        };
        let props = ToolCallStatusViewProps::Request(request);
        let commands = view.render(props, &context);

        assert_eq!(commands.len(), 2);
        assert!(matches!(commands[0], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[1], TerminalCommand::PrintStyled(_)));
    }

    #[test]
    fn test_view_renders_success() {
        let view = ToolCallStatusView {};
        let context = RenderContext::new((0, 0), (0, 0));
        let result = ToolCallResult {
            id: "test-1".to_string(),
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            result: "success".to_string(),
        };
        let props = ToolCallStatusViewProps::Success {
            result,
            line_position: 10,
        };
        let commands = view.render(props, &context);

        assert_eq!(commands.len(), 7);
        assert!(matches!(commands[0], TerminalCommand::SavePosition));
        assert!(matches!(commands[1], TerminalCommand::MoveTo(1, 10)));
        assert!(matches!(commands[2], TerminalCommand::ClearLine));
        assert!(matches!(commands[3], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[4], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[5], TerminalCommand::Print(ref s) if s == "\r\n"));
        assert!(matches!(commands[6], TerminalCommand::RestorePosition));
    }

    #[test]
    fn test_view_renders_error() {
        let view = ToolCallStatusView {};
        let context = RenderContext::new((0, 0), (0, 0));
        let error = ToolCallError {
            id: "test-1".to_string(),
            name: "TestTool".to_string(),
            arguments: Some("test args".to_string()),
            error: "error message".to_string(),
        };
        let props = ToolCallStatusViewProps::Error {
            error,
            line_position: 15,
        };
        let commands = view.render(props, &context);

        assert_eq!(commands.len(), 8);
        assert!(matches!(commands[0], TerminalCommand::SavePosition));
        assert!(matches!(commands[1], TerminalCommand::MoveTo(1, 15)));
        assert!(matches!(commands[2], TerminalCommand::ClearLine));
        assert!(matches!(commands[3], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[4], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[5], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[6], TerminalCommand::Print(ref s) if s == "\r\n"));
        assert!(matches!(commands[7], TerminalCommand::RestorePosition));
    }

    #[test]
    fn test_view_renders_error_without_arguments() {
        let view = ToolCallStatusView {};
        let context = RenderContext::new((0, 0), (0, 0));
        let error = ToolCallError {
            id: "test-1".to_string(),
            name: "TestTool".to_string(),
            arguments: None,
            error: "error message".to_string(),
        };
        let props = ToolCallStatusViewProps::Error {
            error,
            line_position: 20,
        };
        let commands = view.render(props, &context);

        assert_eq!(commands.len(), 8);
        assert!(matches!(commands[0], TerminalCommand::SavePosition));
        assert!(matches!(commands[1], TerminalCommand::MoveTo(1, 20)));
        assert!(matches!(commands[2], TerminalCommand::ClearLine));
        assert!(matches!(commands[3], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[4], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[5], TerminalCommand::PrintStyled(_)));
        assert!(matches!(commands[6], TerminalCommand::Print(ref s) if s == "\r\n"));
        assert!(matches!(commands[7], TerminalCommand::RestorePosition));
    }
}

# Task 06: UI Permission Prompt Component

## Overview
Create the UI component that displays permission prompts to users, showing tool details and diffs, and handles user responses.

## Dependencies
- Task 02: Core Hook Infrastructure must be completed
- Task 04: Diff Generation and Rendering must be completed
- Task 05: Hook Manager and Registry must be completed

## Deliverables

### 1. Permission Prompt Component (`src/components/permission_prompt.rs`)

Create a full-featured permission prompt component:

```rust
use crate::{
    action::{Action, PermissionResponse},
    components::{Component, DiffView},
    hooks::HookResultContext,
};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};
use tokio::sync::oneshot;

#[derive(Debug)]
pub struct PermissionPrompt {
    tool_name: String,
    message: String,
    context: Option<HookResultContext>,
    callback: Option<oneshot::Sender<PermissionResponse>>,
    diff_view: Option<DiffView>,
    mode: PromptMode,
    feedback_input: String,
    cursor_position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PromptMode {
    Viewing,
    ProvidingFeedback,
}

impl PermissionPrompt {
    pub fn new(
        tool_name: String,
        message: String,
        context: Option<HookResultContext>,
        callback: oneshot::Sender<PermissionResponse>,
    ) -> Self {
        // Create diff view if we have file modification context
        let diff_view = match &context {
            Some(HookResultContext::FileModification { diff, path, .. }) => {
                Some(DiffView::new(diff).with_file_path(path.clone()))
            }
            _ => None,
        };
        
        Self {
            tool_name,
            message,
            context,
            callback: Some(callback),
            diff_view,
            mode: PromptMode::Viewing,
            feedback_input: String::new(),
            cursor_position: 0,
        }
    }
    
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let header_lines = vec![
            Line::from(vec![
                Span::raw("Tool: "),
                Span::styled(
                    &self.tool_name,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(&self.message),
        ];
        
        let header = Paragraph::new(header_lines)
            .block(
                Block::default()
                    .title(" Permission Request ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });
            
        header.render(area, buf);
    }
    
    fn render_context(&mut self, area: Rect, buf: &mut Buffer) {
        match &self.context {
            Some(HookResultContext::FileModification { operation, .. }) => {
                // Render operation type badge
                let op_text = match operation {
                    crate::hooks::FileOperation::Create => " CREATE ",
                    crate::hooks::FileOperation::Modify => " MODIFY ",
                    crate::hooks::FileOperation::Delete => " DELETE ",
                    crate::hooks::FileOperation::Rename { .. } => " RENAME ",
                };
                
                let op_color = match operation {
                    crate::hooks::FileOperation::Create => Color::Green,
                    crate::hooks::FileOperation::Modify => Color::Yellow,
                    crate::hooks::FileOperation::Delete => Color::Red,
                    crate::hooks::FileOperation::Rename { .. } => Color::Blue,
                };
                
                let badge = Paragraph::new(op_text)
                    .style(
                        Style::default()
                            .bg(op_color)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    );
                
                // Layout for badge and diff
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Min(0),
                    ])
                    .split(area);
                
                badge.render(chunks[0], buf);
                
                // Render diff view
                if let Some(diff_view) = &mut self.diff_view {
                    diff_view.render(chunks[1], buf);
                }
            }
            
            Some(HookResultContext::CommandExecution { command, args, working_dir }) => {
                let mut lines = vec![
                    Line::from(vec![
                        Span::raw("Command: "),
                        Span::styled(command, Style::default().fg(Color::Cyan)),
                    ]),
                ];
                
                if !args.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw("Arguments: "),
                        Span::styled(args.join(" "), Style::default().fg(Color::Cyan)),
                    ]));
                }
                
                if let Some(dir) = working_dir {
                    lines.push(Line::from(vec![
                        Span::raw("Working Directory: "),
                        Span::styled(dir, Style::default().fg(Color::Cyan)),
                    ]));
                }
                
                let content = Paragraph::new(lines)
                    .block(Block::default().title("Command Details").borders(Borders::ALL));
                content.render(area, buf);
            }
            
            Some(HookResultContext::NetworkRequest { url, method, .. }) => {
                let lines = vec![
                    Line::from(vec![
                        Span::raw("Method: "),
                        Span::styled(method, Style::default().fg(Color::Cyan)),
                    ]),
                    Line::from(vec![
                        Span::raw("URL: "),
                        Span::styled(url, Style::default().fg(Color::Cyan)),
                    ]),
                ];
                
                let content = Paragraph::new(lines)
                    .block(Block::default().title("Network Request").borders(Borders::ALL));
                content.render(area, buf);
            }
            
            _ => {
                let placeholder = Paragraph::new("No additional context available")
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::DarkGray));
                placeholder.render(area, buf);
            }
        }
    }
    
    fn render_controls(&self, area: Rect, buf: &mut Buffer) {
        let controls = match self.mode {
            PromptMode::Viewing => {
                vec![
                    Line::from(vec![
                        Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" - Approve"),
                        Span::raw("  "),
                        Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw(" - Deny"),
                        Span::raw("  "),
                        Span::styled("f", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" - Deny with feedback"),
                    ]),
                    Line::from(vec![
                        Span::styled("↑↓/jk", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(" - Scroll"),
                        Span::raw("  "),
                        Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(" - Cancel"),
                    ]),
                ]
            }
            PromptMode::ProvidingFeedback => {
                vec![
                    Line::from("Provide feedback for denial:"),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(" - Submit"),
                        Span::raw("  "),
                        Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(" - Cancel feedback"),
                    ]),
                ]
            }
        };
        
        let controls_widget = Paragraph::new(controls)
            .block(
                Block::default()
                    .title(" Controls ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .alignment(Alignment::Center);
            
        controls_widget.render(area, buf);
    }
    
    fn render_feedback_input(&self, area: Rect, buf: &mut Buffer) {
        if self.mode == PromptMode::ProvidingFeedback {
            let input = Paragraph::new(self.feedback_input.as_str())
                .block(
                    Block::default()
                        .title(" Feedback ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .style(Style::default().fg(Color::White));
                
            input.render(area, buf);
            
            // Show cursor
            if let Some(cursor_x) = self.feedback_input.chars().take(self.cursor_position).count().checked_add(area.x + 1) {
                if cursor_x < area.x + area.width - 1 {
                    buf.get_mut(cursor_x as u16, area.y + 1)
                        .set_style(Style::default().add_modifier(Modifier::REVERSED));
                }
            }
        }
    }
    
    fn approve(&mut self) -> Result<Option<Action>> {
        if let Some(callback) = self.callback.take() {
            let _ = callback.send(PermissionResponse::Approved);
        }
        Ok(Some(Action::DismissPermissionPrompt))
    }
    
    fn deny(&mut self) -> Result<Option<Action>> {
        if let Some(callback) = self.callback.take() {
            let _ = callback.send(PermissionResponse::Denied);
        }
        Ok(Some(Action::DismissPermissionPrompt))
    }
    
    fn deny_with_feedback(&mut self) -> Result<Option<Action>> {
        if let Some(callback) = self.callback.take() {
            let _ = callback.send(PermissionResponse::DeniedWithFeedback(
                self.feedback_input.clone()
            ));
        }
        Ok(Some(Action::DismissPermissionPrompt))
    }
}

impl Component for PermissionPrompt {
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match self.mode {
            PromptMode::Viewing => match key.code {
                KeyCode::Char('y') => self.approve(),
                KeyCode::Char('n') => self.deny(),
                KeyCode::Char('f') => {
                    self.mode = PromptMode::ProvidingFeedback;
                    self.feedback_input.clear();
                    self.cursor_position = 0;
                    Ok(None)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(diff_view) = &mut self.diff_view {
                        diff_view.scroll_up();
                    }
                    Ok(None)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(diff_view) = &mut self.diff_view {
                        diff_view.scroll_down();
                    }
                    Ok(None)
                }
                KeyCode::Esc => self.deny(),
                _ => Ok(None),
            },
            
            PromptMode::ProvidingFeedback => match key.code {
                KeyCode::Enter => self.deny_with_feedback(),
                KeyCode::Esc => {
                    self.mode = PromptMode::Viewing;
                    Ok(None)
                }
                KeyCode::Backspace => {
                    if self.cursor_position > 0 {
                        self.feedback_input.remove(self.cursor_position - 1);
                        self.cursor_position -= 1;
                    }
                    Ok(None)
                }
                KeyCode::Left => {
                    self.cursor_position = self.cursor_position.saturating_sub(1);
                    Ok(None)
                }
                KeyCode::Right => {
                    if self.cursor_position < self.feedback_input.len() {
                        self.cursor_position += 1;
                    }
                    Ok(None)
                }
                KeyCode::Char(c) => {
                    self.feedback_input.insert(self.cursor_position, c);
                    self.cursor_position += 1;
                    Ok(None)
                }
                _ => Ok(None),
            },
        }
    }
    
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Clear the area first (for overlay effect)
        Clear.render(area, buf);
        
        // Create a centered modal
        let modal_width = area.width.min(100).max(60);
        let modal_height = area.height.min(40).max(20);
        
        let modal_area = Rect {
            x: area.x + (area.width - modal_width) / 2,
            y: area.y + (area.height - modal_height) / 2,
            width: modal_width,
            height: modal_height,
        };
        
        // Draw modal background
        let modal_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));
        modal_block.render(modal_area, buf);
        
        // Layout inside modal
        let inner = modal_block.inner(modal_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(match self.mode {
                PromptMode::Viewing => vec![
                    Constraint::Length(5),    // Header
                    Constraint::Min(0),       // Context/Diff
                    Constraint::Length(4),    // Controls
                ],
                PromptMode::ProvidingFeedback => vec![
                    Constraint::Length(5),    // Header
                    Constraint::Min(0),       // Context/Diff
                    Constraint::Length(3),    // Feedback input
                    Constraint::Length(4),    // Controls
                ],
            })
            .split(inner);
        
        // Render sections
        self.render_header(chunks[0], buf);
        self.render_context(chunks[1], buf);
        
        match self.mode {
            PromptMode::Viewing => {
                self.render_controls(chunks[2], buf);
            }
            PromptMode::ProvidingFeedback => {
                self.render_feedback_input(chunks[2], buf);
                self.render_controls(chunks[3], buf);
            }
        }
    }
}
```

### 2. Integration with App State

Update the main app to handle permission prompts:

```rust
// In src/app.rs, add to App struct:
pub struct App {
    // ... existing fields ...
    permission_prompt: Option<PermissionPrompt>,
}

// In App::update method, handle the new actions:
impl App {
    pub fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::PromptPermission { tool_name, message, context, callback } => {
                self.permission_prompt = Some(PermissionPrompt::new(
                    tool_name,
                    message,
                    context,
                    callback,
                ));
                Ok(None)
            }
            Action::DismissPermissionPrompt => {
                self.permission_prompt = None;
                Ok(None)
            }
            // ... handle other actions ...
        }
        Ok(None)
    }
    
    // In render method, render permission prompt as overlay:
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Render normal UI
        // ... existing render code ...
        
        // Render permission prompt as overlay if present
        if let Some(prompt) = &mut self.permission_prompt {
            prompt.render(area, buf);
        }
    }
    
    // In handle_key_event, give priority to permission prompt:
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        // If permission prompt is active, it handles all input
        if let Some(prompt) = &mut self.permission_prompt {
            return prompt.handle_key_event(key);
        }
        
        // ... existing key handling ...
    }
}
```

### 3. Export Component

Update `src/components/mod.rs`:
```rust
// ... existing exports ...
mod permission_prompt;
pub use permission_prompt::PermissionPrompt;
```

## Testing Requirements

### Manual Testing Checklist

1. **Visual Testing**:
   - [ ] Modal appears centered on screen
   - [ ] All text is readable and properly formatted
   - [ ] Colors are appropriate (yellow border for attention)
   - [ ] Diff rendering shows proper syntax highlighting
   
2. **Interaction Testing**:
   - [ ] 'y' key approves and closes prompt
   - [ ] 'n' key denies and closes prompt
   - [ ] 'f' key switches to feedback mode
   - [ ] ESC key denies/cancels
   - [ ] Arrow keys scroll diff view
   
3. **Feedback Mode**:
   - [ ] Text input works correctly
   - [ ] Cursor is visible and moves properly
   - [ ] Backspace deletes characters
   - [ ] Enter submits feedback

### Integration Test

```rust
#[tokio::test]
async fn test_permission_prompt_flow() {
    use tokio::sync::oneshot;
    
    let (tx, rx) = oneshot::channel();
    let context = Some(HookResultContext::FileModification {
        path: "/test/file.rs".to_string(),
        diff: "--- a/file.rs\n+++ b/file.rs\n@@ -1,1 +1,1 @@\n-old\n+new".to_string(),
        operation: FileOperation::Modify,
    });
    
    let mut prompt = PermissionPrompt::new(
        "write_file".to_string(),
        "Allow write_file to modify /test/file.rs?".to_string(),
        context,
        tx,
    );
    
    // Simulate user pressing 'y'
    let action = prompt.handle_key_event(KeyEvent::from(KeyCode::Char('y'))).unwrap();
    assert_eq!(action, Some(Action::DismissPermissionPrompt));
    
    // Check that approval was sent
    let response = rx.await.unwrap();
    assert!(matches!(response, PermissionResponse::Approved));
}
```

## Acceptance Criteria

- [ ] Component displays tool name and message clearly
- [ ] File modifications show diff with proper syntax highlighting
- [ ] Command executions show command details
- [ ] User can approve, deny, or deny with feedback
- [ ] Feedback mode allows text input
- [ ] Modal overlay renders on top of existing UI
- [ ] Scrolling works for long diffs
- [ ] All keyboard shortcuts work as documented
- [ ] Component properly sends responses through callback
- [ ] Integration with main app works correctly

## Notes for Implementation

- Use Clear widget to ensure modal renders on top
- Center the modal for better visibility
- Consider screen size constraints (min/max dimensions)
- Ensure proper cleanup when prompt is dismissed
- Handle edge cases like very long tool names or messages
- The modal should be keyboard-only (no mouse support needed)
- Consider adding animations in the future
- Make sure the diff view scrolling is smooth
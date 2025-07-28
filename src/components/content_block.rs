use chrono::{DateTime, Utc};
use color_eyre::Result;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize};

use crate::{
    theme::Theme,
    types::{ChatMessage, ToolCallState},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentBlock {
    UserMessage {
        content: String,
        timestamp: DateTime<Utc>,
    },
    AssistantMessage {
        content: Vec<ContentElement>,
        timestamp: DateTime<Utc>,
        streaming: bool,
        // Pre-computed display text to avoid rendering on every draw
        display_text: String,
    },
    SystemMessage {
        content: String,
        timestamp: DateTime<Utc>,
    },
    ToolCallBlock {
        id: String,
        name: String,
        params: String,
        timestamp: DateTime<Utc>,
        state: ToolCallState,
    },
    ToolResultBlock {
        tool_call_id: String,
        content: String,
        timestamp: DateTime<Utc>,
        expanded: bool,
    },
    ErrorBlock {
        message: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentElement {
    Text(String),
    CodeBlock {
        language: String,
        code: String,
        expanded: bool,
    },
    InlineCode(String),
    Bold(String),
    Italic(String),
    Link {
        text: String,
        url: String,
    },
}

impl ContentBlock {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            ContentBlock::UserMessage { timestamp, .. } => *timestamp,
            ContentBlock::AssistantMessage { timestamp, .. } => *timestamp,
            ContentBlock::SystemMessage { timestamp, .. } => *timestamp,
            ContentBlock::ToolCallBlock { timestamp, .. } => *timestamp,
            ContentBlock::ToolResultBlock { timestamp, .. } => *timestamp,
            ContentBlock::ErrorBlock { timestamp, .. } => *timestamp,
        }
    }
    
    // Helper function to compute display text from ContentElements
    fn compute_display_text(content: &[ContentElement]) -> String {
        content
            .iter()
            .map(|elem| match elem {
                ContentElement::Text(t) => t.clone(),
                ContentElement::CodeBlock { code, language, .. } => {
                    format!("```{language}\n{code}\n```")
                }
                ContentElement::InlineCode(c) => format!("`{c}`"),
                ContentElement::Bold(b) => format!("**{b}**"),
                ContentElement::Italic(i) => format!("*{i}*"),
                ContentElement::Link { text, url } => format!("[{text}]({url})"),
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn toggle_expansion(&mut self) {
        match self {
            ContentBlock::ToolResultBlock { expanded, .. } => {
                *expanded = !*expanded;
            }
            ContentBlock::AssistantMessage { content, .. } => {
                // Toggle expansion of code blocks within the assistant message
                for element in content {
                    if let ContentElement::CodeBlock { expanded, .. } = element {
                        *expanded = !*expanded;
                    }
                }
            }
            _ => {} // Other block types don't have expansion
        }
    }

    pub fn toggle_code_block_expansion(&mut self, element_id: usize) {
        if let ContentBlock::AssistantMessage { content, .. } = self {
            if let Some(ContentElement::CodeBlock { expanded, .. }) = content.get_mut(element_id) {
                *expanded = !*expanded;
            }
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        selected: bool,
    ) -> Result<()> {
        let _ = selected; // Suppress unused warning for now
        match self {
            ContentBlock::UserMessage { content, timestamp } => {
                self.render_user_message(frame, area, content, timestamp, theme)
            }
            ContentBlock::AssistantMessage {
                content,
                timestamp,
                streaming,
                ..
            } => self.render_assistant_message(frame, area, content, timestamp, *streaming, theme),
            ContentBlock::SystemMessage { content, timestamp } => {
                self.render_system_message(frame, area, content, timestamp, theme)
            }
            ContentBlock::ToolCallBlock {
                id,
                name,
                params,
                timestamp,
                state,
            } => {
                self.render_tool_call_block(frame, area, id, name, params, timestamp, state, theme)
            }
            ContentBlock::ToolResultBlock {
                tool_call_id,
                content,
                timestamp,
                expanded,
            } => self.render_tool_result_block(
                frame,
                area,
                tool_call_id,
                content,
                timestamp,
                *expanded,
                theme,
            ),
            ContentBlock::ErrorBlock { message, timestamp } => {
                self.render_error_block(frame, area, message, timestamp, theme)
            }
        }
    }

    fn render_user_message(
        &self,
        frame: &mut Frame,
        area: Rect,
        content: &str,
        timestamp: &DateTime<Utc>,
        theme: &Theme,
    ) -> Result<()> {
        let header_line = Line::from(vec![
            Span::styled(
                "You",
                Style::default()
                    .fg(theme.user_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::styled(
                timestamp.format("%H:%M:%S").to_string(),
                Style::default().fg(theme.muted),
            ),
            Span::raw(")"),
        ]);

        let content_lines = content
            .lines()
            .map(|line| Line::from(Span::styled(line, Style::default().fg(theme.foreground))))
            .collect::<Vec<_>>();

        let mut all_lines = vec![header_line];
        all_lines.extend(content_lines);

        let text = Text::from(all_lines);
        let block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .border_style(Style::default().fg(theme.user_color))
            .padding(Padding::horizontal(1));

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_assistant_message(
        &self,
        frame: &mut Frame,
        area: Rect,
        content: &[ContentElement],
        timestamp: &DateTime<Utc>,
        streaming: bool,
        theme: &Theme,
    ) -> Result<()> {
        let header_line = Line::from(vec![
            Span::styled(
                "Assistant",
                Style::default()
                    .fg(theme.assistant_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::styled(
                timestamp.format("%H:%M:%S").to_string(),
                Style::default().fg(theme.muted),
            ),
            Span::raw(")"),
        ]);

        let mut all_lines = vec![header_line];

        for element in content {
            all_lines.extend(self.render_content_element(element, theme));
        }

        // Add cursor for streaming
        if streaming {
            if let Some(last_line) = all_lines.last_mut() {
                let mut spans = last_line.spans.clone();
                spans.push(Span::styled(" ▋", Style::default().fg(theme.cursor_color)));
                *last_line = Line::from(spans);
            } else {
                all_lines.push(Line::from(Span::styled(
                    " ▋",
                    Style::default().fg(theme.cursor_color),
                )));
            }
        }

        let text = Text::from(all_lines);
        let block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .border_style(Style::default().fg(theme.assistant_color))
            .padding(Padding::horizontal(1));

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_system_message(
        &self,
        frame: &mut Frame,
        area: Rect,
        content: &str,
        timestamp: &DateTime<Utc>,
        theme: &Theme,
    ) -> Result<()> {
        let header_line = Line::from(vec![
            Span::styled(
                "System",
                Style::default()
                    .fg(theme.system_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::styled(
                timestamp.format("%H:%M:%S").to_string(),
                Style::default().fg(theme.muted),
            ),
            Span::raw(")"),
        ]);

        let content_lines = content
            .lines()
            .map(|line| Line::from(Span::styled(line, Style::default().fg(theme.subtle))))
            .collect::<Vec<_>>();

        let mut all_lines = vec![header_line];
        all_lines.extend(content_lines);

        let text = Text::from(all_lines);
        let block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .border_style(Style::default().fg(theme.system_color))
            .padding(Padding::horizontal(1));

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_tool_call_block(
        &self,
        frame: &mut Frame,
        area: Rect,
        id: &str,
        name: &str,
        params: &str,
        timestamp: &DateTime<Utc>,
        state: &ToolCallState,
        theme: &Theme,
    ) -> Result<()> {
        let state_symbol = match state {
            ToolCallState::Pending => "⏳",
            ToolCallState::Running => "🔄",
            ToolCallState::Completed => "✅",
            ToolCallState::Failed => "❌",
        };

        let header_line = Line::from(vec![
            Span::raw(state_symbol),
            Span::raw(" "),
            Span::styled(
                "Tool Call",
                Style::default()
                    .fg(theme.tool_call_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::styled(id, Style::default().fg(theme.subtle)),
            Span::raw(") "),
            Span::styled(
                timestamp.format("%H:%M:%S").to_string(),
                Style::default().fg(theme.muted),
            ),
        ]);

        let call_line = Line::from(vec![
            Span::styled(
                name,
                Style::default()
                    .fg(theme.foreground)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("("),
            Span::styled(params, Style::default().fg(theme.foreground)),
            Span::raw(")"),
        ]);

        let text = Text::from(vec![header_line, call_line]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.tool_call_color))
            .padding(Padding::horizontal(1));

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_tool_result_block(
        &self,
        frame: &mut Frame,
        area: Rect,
        tool_call_id: &str,
        content: &str,
        timestamp: &DateTime<Utc>,
        expanded: bool,
        theme: &Theme,
    ) -> Result<()> {
        let header_line = Line::from(vec![
            Span::styled(
                "Result",
                Style::default()
                    .fg(theme.tool_result_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::styled(tool_call_id, Style::default().fg(theme.subtle)),
            Span::raw(") "),
            Span::styled(
                timestamp.format("%H:%M:%S").to_string(),
                Style::default().fg(theme.muted),
            ),
            Span::raw(" "),
            Span::styled(
                if expanded { "▼" } else { "▶" },
                Style::default().fg(theme.muted),
            ),
        ]);

        let mut all_lines = vec![header_line];

        if expanded {
            // Show full content
            let content_lines = content
                .lines()
                .map(|line| Line::from(Span::styled(line, Style::default().fg(theme.foreground))))
                .collect::<Vec<_>>();
            all_lines.extend(content_lines);
        } else {
            // Show truncated content
            let preview = if content.len() > 100 {
                format!("{}...", &content[..100])
            } else {
                content.to_string()
            };
            all_lines.push(Line::from(Span::styled(
                preview,
                Style::default().fg(theme.foreground),
            )));
        }

        let text = Text::from(all_lines);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.tool_result_color))
            .padding(Padding::horizontal(1));

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_error_block(
        &self,
        frame: &mut Frame,
        area: Rect,
        message: &str,
        timestamp: &DateTime<Utc>,
        theme: &Theme,
    ) -> Result<()> {
        let header_line = Line::from(vec![
            Span::styled(
                "❌ Error",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::styled(
                timestamp.format("%H:%M:%S").to_string(),
                Style::default().fg(theme.muted),
            ),
            Span::raw(")"),
        ]);

        let content_lines = message
            .lines()
            .map(|line| Line::from(Span::styled(line, Style::default().fg(theme.error))))
            .collect::<Vec<_>>();

        let mut all_lines = vec![header_line];
        all_lines.extend(content_lines);

        let text = Text::from(all_lines);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.error))
            .padding(Padding::horizontal(1));

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_content_element(
        &self,
        element: &ContentElement,
        theme: &Theme,
    ) -> Vec<Line<'static>> {
        match element {
            ContentElement::Text(text) => text
                .lines()
                .map(|line| {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(theme.foreground),
                    ))
                })
                .collect(),
            ContentElement::CodeBlock {
                language,
                code,
                expanded,
            } => {
                let mut lines = vec![];

                // Code block header
                let header = Line::from(vec![
                    Span::styled("```".to_string(), Style::default().fg(theme.muted)),
                    Span::styled(language.clone(), Style::default().fg(theme.warning)),
                    Span::raw(" "),
                    Span::styled(
                        if *expanded { "▼" } else { "▶" }.to_string(),
                        Style::default().fg(theme.muted),
                    ),
                ]);
                lines.push(header);

                if *expanded {
                    // Show full code
                    for code_line in code.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  {code_line}"),
                            Style::default().fg(theme.code_fg).bg(theme.code_bg),
                        )));
                    }
                } else {
                    // Show truncated code
                    let first_line = code.lines().next().unwrap_or("");
                    lines.push(Line::from(Span::styled(
                        format!("  {first_line}..."),
                        Style::default().fg(theme.code_fg).bg(theme.code_bg),
                    )));
                }

                // Code block footer
                lines.push(Line::from(Span::styled(
                    "```".to_string(),
                    Style::default().fg(theme.muted),
                )));
                lines
            }
            ContentElement::InlineCode(code) => {
                vec![Line::from(Span::styled(
                    code.clone(),
                    Style::default().fg(theme.code_fg).bg(theme.code_bg),
                ))]
            }
            ContentElement::Bold(text) => {
                vec![Line::from(Span::styled(
                    text.clone(),
                    Style::default()
                        .fg(theme.foreground)
                        .add_modifier(Modifier::BOLD),
                ))]
            }
            ContentElement::Italic(text) => {
                vec![Line::from(Span::styled(
                    text.clone(),
                    Style::default()
                        .fg(theme.foreground)
                        .add_modifier(Modifier::ITALIC),
                ))]
            }
            ContentElement::Link { text, url: _url } => {
                // For TUI, we'll just show the text with a different color
                vec![Line::from(Span::styled(
                    text.clone(),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                ))]
            }
        }
    }

    pub fn calculate_height(&self, width: u16) -> u16 {
        match self {
            ContentBlock::UserMessage { content, .. } => {
                let header_height = 1;
                let content_height = content.lines().count() as u16;
                let padding = 2; // top and bottom padding
                header_height + content_height + padding
            }
            ContentBlock::AssistantMessage { content, .. } => {
                let header_height = 1;
                let mut content_height = 0;
                for element in content {
                    content_height += self.calculate_element_height(element, width);
                }
                let padding = 2;
                header_height + content_height + padding
            }
            ContentBlock::SystemMessage { content, .. } => {
                let header_height = 1;
                let content_height = content.lines().count() as u16;
                let padding = 2;
                header_height + content_height + padding
            }
            ContentBlock::ToolCallBlock { .. } => {
                4 // header + call line + borders
            }
            ContentBlock::ToolResultBlock {
                content, expanded, ..
            } => {
                let header_height = 1;
                let content_height = if *expanded {
                    content.lines().count() as u16
                } else {
                    1 // truncated preview
                };
                let padding = 2;
                header_height + content_height + padding
            }
            ContentBlock::ErrorBlock { message, .. } => {
                let header_height = 1;
                let content_height = message.lines().count() as u16;
                let padding = 2;
                header_height + content_height + padding
            }
        }
    }

    fn calculate_element_height(&self, element: &ContentElement, _width: u16) -> u16 {
        match element {
            ContentElement::Text(text) => text.lines().count() as u16,
            ContentElement::CodeBlock { code, expanded, .. } => {
                if *expanded {
                    2 + code.lines().count() as u16 // header + footer + content
                } else {
                    3 // header + truncated line + footer
                }
            }
            ContentElement::InlineCode(_) => 1,
            ContentElement::Bold(_) => 1,
            ContentElement::Italic(_) => 1,
            ContentElement::Link { .. } => 1,
        }
    }
}

impl From<&ChatMessage> for ContentBlock {
    fn from(message: &ChatMessage) -> Self {
        match message {
            ChatMessage::User { content, timestamp } => ContentBlock::UserMessage {
                content: content.clone(),
                timestamp: *timestamp,
            },
            ChatMessage::Assistant { content, timestamp } => {
                let parsed_content = parse_assistant_content(content);
                let display_text = Self::compute_display_text(&parsed_content);
                ContentBlock::AssistantMessage {
                    content: parsed_content,
                    timestamp: *timestamp,
                    streaming: false,
                    display_text,
                }
            },
            ChatMessage::AssistantStreaming { content, timestamp } => {
                let parsed_content = parse_assistant_content(content);
                let display_text = Self::compute_display_text(&parsed_content);
                ContentBlock::AssistantMessage {
                    content: parsed_content,
                    timestamp: *timestamp,
                    streaming: true,
                    display_text,
                }
            }
            ChatMessage::System { content, timestamp } => ContentBlock::SystemMessage {
                content: content.clone(),
                timestamp: *timestamp,
            },
            ChatMessage::ToolCall {
                id,
                name,
                params,
                timestamp,
            } => ContentBlock::ToolCallBlock {
                id: id.clone(),
                name: name.clone(),
                params: params.clone(),
                timestamp: *timestamp,
                state: ToolCallState::Running,
            },
            ChatMessage::ToolResult {
                tool_call_id,
                content,
                timestamp,
            } => ContentBlock::ToolResultBlock {
                tool_call_id: tool_call_id.clone(),
                content: content.clone(),
                timestamp: *timestamp,
                expanded: false,
            },
            ChatMessage::Tool {
                tool_call_id,
                content,
                timestamp,
            } => ContentBlock::ToolResultBlock {
                tool_call_id: tool_call_id.clone(),
                content: content.clone(),
                timestamp: *timestamp,
                expanded: false,
            },
            ChatMessage::Error { message, timestamp } => ContentBlock::ErrorBlock {
                message: message.clone(),
                timestamp: *timestamp,
            },
        }
    }
}

fn parse_assistant_content(content: &str) -> Vec<ContentElement> {
    let mut elements = Vec::new();
    let mut current_text = String::new();
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        if line.starts_with("```") {
            // Flush any accumulated text
            if !current_text.is_empty() {
                elements.extend(parse_inline_formatting(&current_text));
                current_text.clear();
            }

            // Parse code block
            let language = line.trim_start_matches("```").to_string();
            let mut code_lines = Vec::new();

            for code_line in lines.by_ref() {
                if code_line.starts_with("```") {
                    break;
                }
                code_lines.push(code_line);
            }

            elements.push(ContentElement::CodeBlock {
                language,
                code: code_lines.join("\n"),
                expanded: true,
            });
        } else {
            current_text.push_str(line);
            current_text.push('\n');
        }
    }

    // Flush any remaining text
    if !current_text.is_empty() {
        elements.extend(parse_inline_formatting(
            &current_text.trim_end_matches('\n'),
        ));
    }

    if elements.is_empty() {
        elements.push(ContentElement::Text(content.to_string()));
    }

    elements
}

fn parse_inline_formatting(text: &str) -> Vec<ContentElement> {
    let mut elements = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                // Bold text **text**
                if !current.is_empty() {
                    elements.push(ContentElement::Text(current.clone()));
                    current.clear();
                }
                chars.next(); // consume second *
                let mut bold_text = String::new();
                let mut found_end = false;

                while let Some(ch) = chars.next() {
                    if ch == '*' && chars.peek() == Some(&'*') {
                        chars.next(); // consume second *
                        found_end = true;
                        break;
                    }
                    bold_text.push(ch);
                }

                if found_end && !bold_text.is_empty() {
                    elements.push(ContentElement::Bold(bold_text));
                } else {
                    current.push_str("**");
                    current.push_str(&bold_text);
                }
            }
            '*' => {
                // Italic text *text*
                if !current.is_empty() {
                    elements.push(ContentElement::Text(current.clone()));
                    current.clear();
                }
                let mut italic_text = String::new();
                let mut found_end = false;

                for ch in chars.by_ref() {
                    if ch == '*' {
                        found_end = true;
                        break;
                    }
                    italic_text.push(ch);
                }

                if found_end && !italic_text.is_empty() {
                    elements.push(ContentElement::Italic(italic_text));
                } else {
                    current.push('*');
                    current.push_str(&italic_text);
                }
            }
            '`' => {
                // Inline code `code`
                if !current.is_empty() {
                    elements.push(ContentElement::Text(current.clone()));
                    current.clear();
                }
                let mut code_text = String::new();
                let mut found_end = false;

                for ch in chars.by_ref() {
                    if ch == '`' {
                        found_end = true;
                        break;
                    }
                    code_text.push(ch);
                }

                if found_end && !code_text.is_empty() {
                    elements.push(ContentElement::InlineCode(code_text));
                } else {
                    current.push('`');
                    current.push_str(&code_text);
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        elements.push(ContentElement::Text(current));
    }

    elements
}

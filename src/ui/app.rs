use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use tokio::sync::mpsc;

use crate::llm::LlmProvider;
use crate::mcp::McpClient;

#[derive(Debug, Clone)]
pub enum UiMessage {
    User { content: String },
    Assistant { content: String },
    ToolCall { name: String, params: String },
    ToolResult { content: String },
    Error { message: String },
}

pub struct App {
    pub messages: Vec<UiMessage>,
    pub input: String,
    pub scroll_offset: u16,
    pub is_running: bool,
    llm_provider: Box<dyn LlmProvider>,
    mcp_client: McpClient,
}

impl App {
    pub fn new(llm_provider: Box<dyn LlmProvider>, mcp_client: McpClient) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            scroll_offset: 0,
            is_running: true,
            llm_provider,
            mcp_client,
        }
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        while self.is_running {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                self.handle_key_event(key).await?;
            }
        }
        Ok(())
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.is_running = false;
            }
            KeyCode::Enter => {
                self.handle_input().await?;
            }
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Up => {
                self.scroll_up();
            }
            KeyCode::Down => {
                self.scroll_down();
            }
            KeyCode::PageUp => {
                self.page_up();
            }
            KeyCode::PageDown => {
                self.page_down();
            }
            _ => {}
        }
        Ok(())
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
    }

    fn page_down(&mut self) {
        self.scroll_offset += 10;
    }

    fn render(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),      // Chat area
                Constraint::Length(3),   // Input area
            ])
            .split(f.area());

        self.render_chat_area(f, chunks[0]);
        self.render_input_area(f, chunks[1]);
    }

    fn render_chat_area(&self, f: &mut Frame, area: Rect) {
        let messages: Vec<ListItem> = self.messages
            .iter()
            .skip(self.scroll_offset as usize)
            .map(|msg| self.message_to_list_item(msg))
            .collect();

        let list = List::new(messages)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Chat")
            );

        f.render_widget(list, area);
    }

    fn render_input_area(&self, f: &mut Frame, area: Rect) {
        let input = Paragraph::new(self.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Input")
            )
            .wrap(Wrap { trim: true });

        f.render_widget(input, area);
    }

    fn message_to_list_item(&self, message: &UiMessage) -> ListItem<'static> {
        match message {
            UiMessage::User { content } => {
                ListItem::new(Line::from(vec![
                    Span::styled("You: ".to_string(), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    Span::raw(content.clone()),
                ]))
            }
            UiMessage::Assistant { content } => {
                ListItem::new(Line::from(vec![
                    Span::styled("Assistant: ".to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(content.clone()),
                ]))
            }
            UiMessage::ToolCall { name, params } => {
                ListItem::new(Line::from(vec![
                    Span::styled("Tool: ".to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw(format!("{}({})", name, params)),
                ]))
            }
            UiMessage::ToolResult { content } => {
                ListItem::new(Line::from(vec![
                    Span::styled("Result: ".to_string(), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                    Span::raw(content.clone()),
                ]))
            }
            UiMessage::Error { message } => {
                ListItem::new(Line::from(vec![
                    Span::styled("Error: ".to_string(), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::raw(message.clone()),
                ]))
            }
        }
    }

    async fn handle_input(&mut self) -> Result<()> {
        if !self.input.trim().is_empty() {
            let user_input = self.input.clone();
            self.messages.push(UiMessage::User { content: user_input });
            self.input.clear();
            
            // TODO: Send to LLM and get response
            // For now, just add a placeholder response
            self.messages.push(UiMessage::Assistant { 
                content: "LLM integration not yet implemented".to_string() 
            });
        }
        Ok(())
    }

    pub fn add_message(&mut self, message: UiMessage) {
        self.messages.push(message);
    }
}
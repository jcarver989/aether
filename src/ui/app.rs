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
use tokio_stream::StreamExt;
use tokio::time::{timeout, Duration};

use crate::llm::{LlmProvider, ChatRequest, ChatMessage, ToolDefinition};
use crate::llm::provider::StreamChunk;
use crate::mcp::McpClient;

#[derive(Debug, Clone)]
pub enum UiMessage {
    User { content: String },
    Assistant { content: String },
    AssistantStreaming { content: String },
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
    agent_context: Option<String>,
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
            agent_context: None,
        }
    }

    pub fn with_agent_context(mut self, agent_context: Option<String>) -> Self {
        self.agent_context = agent_context;
        self
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
            UiMessage::AssistantStreaming { content } => {
                ListItem::new(Line::from(vec![
                    Span::styled("Assistant: ".to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(content.clone()),
                    Span::styled(" ▋".to_string(), Style::default().fg(Color::Gray)), // cursor indicator
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
            self.messages.push(UiMessage::User { content: user_input.clone() });
            self.input.clear();
            
            // Send to LLM and handle response
            match self.send_to_llm(user_input).await {
                Ok(_) => {},
                Err(e) => {
                    self.messages.push(UiMessage::Error { 
                        message: format!("LLM error: {}", e) 
                    });
                }
            }
        }
        Ok(())
    }

    async fn send_to_llm(&mut self, user_input: String) -> Result<()> {
        // Build chat context
        let mut chat_messages = Vec::new();
        
        // Add system prompt with agent context if available
        let system_prompt = if let Some(agent_context) = &self.agent_context {
            format!("You are an AI assistant. Here are your instructions:\n\n{}", agent_context)
        } else {
            "You are an AI assistant.".to_string()
        };
        chat_messages.push(ChatMessage::System { content: system_prompt });
        
        // Add conversation history
        for message in &self.messages {
            match message {
                UiMessage::User { content } => {
                    chat_messages.push(ChatMessage::User { content: content.clone() });
                }
                UiMessage::Assistant { content } | UiMessage::AssistantStreaming { content } => {
                    chat_messages.push(ChatMessage::Assistant { content: content.clone() });
                }
                // Skip other message types for now
                _ => {}
            }
        }
        
        // Get available tools from MCP
        let available_tools = self.mcp_client.get_available_tools();
        let tool_definitions: Vec<ToolDefinition> = available_tools.iter()
            .filter_map(|tool_name| {
                let description = self.mcp_client.get_tool_description(tool_name)?;
                let parameters = self.mcp_client.get_tool_parameters(tool_name)
                    .map(|p| p.clone())
                    .unwrap_or_else(|| serde_json::json!({}));
                
                Some(ToolDefinition {
                    name: tool_name.clone(),
                    description,
                    parameters,
                })
            })
            .collect();
        
        // Send to LLM with streaming
        let request = ChatRequest {
            messages: chat_messages,
            tools: tool_definitions,
            temperature: Some(0.7),
        };
        
        let mut stream = self.llm_provider.complete_stream_chunks(request).await?;
        let mut current_content = String::new();
        let mut current_tool_calls = std::collections::HashMap::<String, (String, String)>::new();
        let streaming_message_index = self.messages.len();
        
        // Add initial streaming message
        self.messages.push(UiMessage::AssistantStreaming { content: String::new() });
        
        loop {
            let chunk_result = match timeout(Duration::from_secs(30), stream.next()).await {
                Ok(Some(result)) => result,
                Ok(None) => {
                    // Stream ended normally
                    if let Some(msg) = self.messages.get_mut(streaming_message_index) {
                        *msg = UiMessage::Assistant { content: current_content };
                    }
                    break;
                }
                Err(_) => {
                    // Timeout
                    self.messages.push(UiMessage::Error { 
                        message: "Stream timeout after 30 seconds".to_string() 
                    });
                    if let Some(msg) = self.messages.get_mut(streaming_message_index) {
                        *msg = UiMessage::Assistant { content: current_content };
                    }
                    return Err(anyhow::anyhow!("Stream timeout"));
                }
            };
            let chunk = match chunk_result {
                Ok(chunk) => chunk,
                Err(e) => {
                    // Handle stream interruption
                    self.messages.push(UiMessage::Error { 
                        message: format!("Stream interrupted: {}", e) 
                    });
                    // Convert partial streaming message to final message
                    if let Some(msg) = self.messages.get_mut(streaming_message_index) {
                        *msg = UiMessage::Assistant { content: current_content };
                    }
                    return Err(e);
                }
            };
            
            match chunk {
                StreamChunk::Content(content) => {
                    current_content.push_str(&content);
                    // Update the streaming message
                    if let Some(msg) = self.messages.get_mut(streaming_message_index) {
                        *msg = UiMessage::AssistantStreaming { content: current_content.clone() };
                    }
                }
                StreamChunk::ToolCallStart { id, name } => {
                    current_tool_calls.insert(id.clone(), (name.clone(), String::new()));
                    self.messages.push(UiMessage::ToolCall { 
                        name: name,
                        params: String::new(),
                    });
                }
                StreamChunk::ToolCallArgument { id, argument } => {
                    if let Some((name, args)) = current_tool_calls.get_mut(&id) {
                        args.push_str(&argument);
                        // Update the tool call message
                        if let Some(UiMessage::ToolCall { params, .. }) = self.messages.last_mut() {
                            *params = args.clone();
                        }
                    }
                }
                StreamChunk::ToolCallComplete { id } => {
                    if let Some((name, args)) = current_tool_calls.remove(&id) {
                        // Parse and execute the tool call
                        let args_json: serde_json::Value = serde_json::from_str(&args)
                            .unwrap_or_else(|_| serde_json::json!({}));
                        
                        match self.mcp_client.execute_tool(&name, args_json).await {
                            Ok(result) => {
                                self.messages.push(UiMessage::ToolResult { 
                                    content: result.to_string() 
                                });
                            }
                            Err(e) => {
                                self.messages.push(UiMessage::Error { 
                                    message: format!("Tool execution failed: {}", e) 
                                });
                            }
                        }
                    }
                }
                StreamChunk::Done => {
                    // Convert streaming message to final message
                    if let Some(msg) = self.messages.get_mut(streaming_message_index) {
                        *msg = UiMessage::Assistant { content: current_content };
                    }
                    break;
                }
            }
        }
        
        Ok(())
    }

    pub fn add_message(&mut self, message: UiMessage) {
        self.messages.push(message);
    }
}
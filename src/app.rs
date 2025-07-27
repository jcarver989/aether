use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{debug, info, error, warn};

use crate::{
    action::Action,
    cli::Cli,
    components::{Component, fps::FpsCounter, home::Home},
    config::{Config, AppConfig},
    llm::{LlmProvider, ChatRequest, ChatMessage as LlmChatMessage, ToolDefinition},
    mcp::McpClient,
    tui::{Event, Tui},
    types::ChatMessage,
};

pub struct App {
    config: Config,
    app_config: AppConfig,
    components: Vec<Box<dyn Component>>,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,
    llm_provider: Option<Box<dyn LlmProvider>>,
    mcp_client: Option<McpClient>,
    active_tool_calls: HashMap<String, PartialToolCall>,
}

#[derive(Debug, Clone)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Home,
}

impl App {
    pub fn new(cli_args: &Cli) -> Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        
        // Load configuration with CLI args
        let config = Config::with_cli_args(Some(cli_args))?;
        let app_config = config.config.clone();
        
        // Initialize LLM provider from configuration
        let llm_provider = match Self::create_provider(&app_config.llm) {
            Ok(provider) => Some(provider),
            Err(e) => {
                error!("Failed to initialize LLM provider: {}", e);
                None
            }
        };
        
        // Initialize MCP client
        let mcp_client = McpClient::new();
        
        Ok(Self {
            components: vec![
                Box::new(Home::new()),
                Box::new(FpsCounter::default())
            ],
            should_quit: false,
            should_suspend: false,
            config,
            app_config,
            mode: Mode::Home,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
            llm_provider,
            mcp_client: Some(mcp_client),
            active_tool_calls: HashMap::new(),
        })
    }
    
    fn create_provider(llm_config: &crate::config::LlmConfig) -> Result<Box<dyn LlmProvider>> {
        use crate::llm::{openrouter::OpenRouterProvider, ollama::OllamaProvider};
        use crate::config::ProviderType;
        
        match llm_config.provider {
            ProviderType::OpenRouter => {
                let api_key = llm_config.openrouter_api_key.as_ref()
                    .ok_or_else(|| color_eyre::Report::msg("OpenRouter API key not found"))?;
                let provider = OpenRouterProvider::new(api_key.clone(), llm_config.model.clone())
                    .map_err(|e| color_eyre::Report::msg(format!("Failed to create OpenRouter provider: {}", e)))?;
                Ok(Box::new(provider))
            }
            ProviderType::Ollama => {
                let provider = OllamaProvider::new(Some(llm_config.ollama_base_url.clone()), llm_config.model.clone())
                    .map_err(|e| color_eyre::Report::msg(format!("Failed to create Ollama provider: {}", e)))?;
                Ok(Box::new(provider))
            }
        }
    }

    async fn initialize_mcp_client(&mut self) -> Result<()> {
        let mcp_client = match &mut self.mcp_client {
            Some(client) => client,
            None => {
                warn!("MCP client not available");
                return Ok(());
            }
        };

        // Use MCP configuration from app config
        let config = &self.app_config.mcp.servers;
        
        if config.is_empty() {
            warn!("No MCP servers configured");
            self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                message: "No MCP servers configured".to_string(),
                timestamp: chrono::Utc::now(),
            }))?;
            return Ok(());
        }

        let mut connected_servers = 0;
        
        // Connect to configured servers with timeout
        for (name, server_config) in config {
            let connect_timeout = std::time::Duration::from_secs(10);
            let connect_future = mcp_client.connect_server(name.clone(), server_config.clone());
            
            match tokio::time::timeout(connect_timeout, connect_future).await {
                Ok(Ok(())) => {
                    info!("Successfully connected to MCP server: {}", name);
                    connected_servers += 1;
                }
                Ok(Err(e)) => {
                    error!("Failed to connect to MCP server {}: {}", name, e);
                    // Send notification but continue with other servers
                    self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                        message: format!("Failed to connect to MCP server '{}': {}", name, e),
                        timestamp: chrono::Utc::now(),
                    }))?;
                }
                Err(_) => {
                    error!("Connection to MCP server {} timed out", name);
                    self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                        message: format!("Connection to MCP server '{}' timed out", name),
                        timestamp: chrono::Utc::now(),
                    }))?;
                }
            }
        }

        if connected_servers == 0 {
            warn!("No MCP servers successfully connected");
            self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                message: "No MCP servers could be connected. Tool functionality will be limited.".to_string(),
                timestamp: chrono::Utc::now(),
            }))?;
            return Ok(());
        }

        // Discover tools from all connected servers
        match mcp_client.discover_tools().await {
            Ok(()) => {
                let tool_count = mcp_client.get_available_tools().len();
                info!("Successfully discovered {} tools from {} servers", tool_count, connected_servers);
                self.action_tx.send(Action::AddChatMessage(ChatMessage::Assistant { 
                    content: format!("Connected to {} MCP servers and discovered {} tools", connected_servers, tool_count),
                    timestamp: chrono::Utc::now(),
                }))?;
            }
            Err(e) => {
                error!("Failed to discover tools: {}", e);
                self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                    message: format!("Failed to discover tools: {}", e),
                    timestamp: chrono::Utc::now(),
                }))?;
            }
        }

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        // Initialize MCP client and connect to servers
        if let Err(e) = self.initialize_mcp_client().await {
            error!("Failed to initialize MCP client: {}", e);
        }

        let mut tui = Tui::new()?
            // .mouse(true) // uncomment this line to enable mouse support
            .tick_rate(self.app_config.ui.tick_rate)
            .frame_rate(self.app_config.ui.frame_rate);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(self.action_tx.clone())?;
        }
        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }
        for component in self.components.iter_mut() {
            component.init(tui.size()?)?;
        }

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            self.handle_actions(&mut tui).await?;
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                action_tx.send(Action::ClearScreen)?;
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    async fn handle_events(&mut self, tui: &mut Tui) -> Result<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        match event {
            Event::Quit => action_tx.send(Action::Quit)?,
            Event::Tick => action_tx.send(Action::Tick)?,
            Event::Render => action_tx.send(Action::Render)?,
            Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
            Event::Key(key) => self.handle_key_event(key)?,
            _ => {}
        }
        for component in self.components.iter_mut() {
            if let Some(action) = component.handle_events(Some(event.clone()))? {
                action_tx.send(action)?;
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        let action_tx = self.action_tx.clone();
        let Some(keymap) = self.config.keybindings.get(&self.mode) else {
            return Ok(());
        };
        match keymap.get(&vec![key]) {
            Some(action) => {
                info!("Got action: {action:?}");
                action_tx.send(action.clone())?;
            }
            _ => {
                // If the key was not handled as a single key action,
                // then consider it for multi-key combinations.
                self.last_tick_key_events.push(key);

                // Check for multi-key combinations
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
                }
            }
        }
        Ok(())
    }

    async fn handle_actions(&mut self, tui: &mut Tui) -> Result<()> {
        while let Ok(action) = self.action_rx.try_recv() {
            if action != Action::Tick && action != Action::Render {
                debug!("{action:?}");
            }
            match action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize(w, h) => self.handle_resize(tui, w, h)?,
                Action::Render => self.render(tui)?,
                Action::SubmitMessage(ref message) => {
                    self.handle_submit_message(message.clone()).await?;
                }
                Action::ReceiveStreamChunk(ref chunk) => {
                    self.handle_stream_chunk(chunk.clone()).await?;
                }
                Action::ExecuteToolCall(ref tool_call) => {
                    self.handle_execute_tool_call(tool_call.clone()).await?;
                }
                Action::ReceiveAssistantMessage(ref message) => {
                    self.action_tx.send(Action::AddChatMessage(ChatMessage::Assistant { 
                        content: message.clone(),
                        timestamp: chrono::Utc::now(),
                    }))?;
                }
                Action::ToolExecutionResult { ref result } => {
                    self.action_tx.send(Action::AddChatMessage(ChatMessage::ToolResult { 
                        content: result.clone(),
                        timestamp: chrono::Utc::now(),
                    }))?;
                }
                Action::RefreshTools => {
                    self.handle_refresh_tools().await?;
                }
                _ => {}
            }
            for component in self.components.iter_mut() {
                if let Some(action) = component.update(action.clone())? {
                    self.action_tx.send(action)?
                };
            }
        }
        Ok(())
    }

    fn handle_resize(&mut self, tui: &mut Tui, w: u16, h: u16) -> Result<()> {
        tui.resize(Rect::new(0, 0, w, h))?;
        self.render(tui)?;
        Ok(())
    }

    fn render(&mut self, tui: &mut Tui) -> Result<()> {
        tui.draw(|frame| {
            for component in self.components.iter_mut() {
                if let Err(err) = component.draw(frame, frame.area()) {
                    let _ = self
                        .action_tx
                        .send(Action::Error(format!("Failed to draw: {:?}", err)));
                }
            }
        })?;
        Ok(())
    }

    async fn handle_submit_message(&mut self, user_input: String) -> Result<()> {
        debug!("Handling user message: {}", user_input);
        
        let llm_provider = match &self.llm_provider {
            Some(provider) => provider,
            None => {
                self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                    message: "LLM provider not initialized".to_string(),
                    timestamp: chrono::Utc::now(),
                }))?;
                return Ok(());
            }
        };
        
        let mcp_client = match &self.mcp_client {
            Some(client) => client,
            None => {
                self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                    message: "MCP client not initialized".to_string(),
                    timestamp: chrono::Utc::now(),
                }))?;
                return Ok(());
            }
        };
        
        // Add user message to chat
        self.action_tx.send(Action::AddChatMessage(ChatMessage::User { 
            content: user_input.clone(),
            timestamp: chrono::Utc::now(),
        }))?;
        
        // Start streaming
        self.action_tx.send(Action::StartStreaming)?;
        
        // Build chat context
        let mut chat_messages = Vec::new();
        
        // Add system prompt with agent context if available
        let system_prompt = if let Some(agent_context) = &self.app_config.agent_context {
            format!("You are an AI assistant. Here are your instructions:\n\n{}", agent_context)
        } else {
            "You are an AI assistant.".to_string()
        };
        chat_messages.push(LlmChatMessage::System { content: system_prompt });
        chat_messages.push(LlmChatMessage::User { content: user_input });
        
        // Get available tools from MCP
        let available_tools = mcp_client.get_available_tools();
        let tool_definitions: Vec<ToolDefinition> = available_tools.iter()
            .filter_map(|tool_name| {
                let description = mcp_client.get_tool_description(tool_name)?;
                let parameters = mcp_client.get_tool_parameters(tool_name)
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
        
        match llm_provider.complete_stream_chunks(request).await {
            Ok(stream) => {
                let tx_clone = self.action_tx.clone();
                let _mcp_client = mcp_client;
                
                // Spawn background task to handle stream
                tokio::spawn(async move {
                    let mut stream = stream;
                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                // Send the chunk to the new action handler
                                if tx_clone.send(Action::ReceiveStreamChunk(chunk)).is_err() {
                                    break; // Receiver dropped
                                }
                            }
                            Err(e) => {
                                error!("Stream error: {}", e);
                                let _ = tx_clone.send(Action::Error(e.to_string()));
                                break;
                            }
                        }
                    }
                });
            }
            Err(e) => {
                error!("Failed to start LLM stream: {}", e);
                self.action_tx.send(Action::Error(e.to_string()))?;
            }
        }
        
        Ok(())
    }
    
    async fn handle_stream_chunk(&mut self, chunk: crate::llm::provider::StreamChunk) -> Result<()> {
        use crate::llm::provider::StreamChunk;
        
        match chunk {
            StreamChunk::Content(content) => {
                self.action_tx.send(Action::StreamContent(content))?;
            }
            StreamChunk::ToolCallStart { id, name } => {
                // Start tracking this tool call
                self.active_tool_calls.insert(id.clone(), PartialToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: String::new(),
                });
                
                self.action_tx.send(Action::StreamToolCall { 
                    id: id.clone(), 
                    name, 
                    arguments: String::new() 
                })?;
            }
            StreamChunk::ToolCallArgument { id, argument } => {
                // Accumulate arguments for this tool call
                if let Some(partial_call) = self.active_tool_calls.get_mut(&id) {
                    partial_call.arguments.push_str(&argument);
                    
                    // Update the UI with the accumulated arguments
                    self.action_tx.send(Action::StreamToolCall { 
                        id: id.clone(), 
                        name: partial_call.name.clone(),
                        arguments: partial_call.arguments.clone()
                    })?;
                }
            }
            StreamChunk::ToolCallComplete { id } => {
                // Tool call is complete, execute it
                if let Some(partial_call) = self.active_tool_calls.remove(&id) {
                    // Parse the accumulated arguments as JSON
                    match serde_json::from_str(&partial_call.arguments) {
                        Ok(mut arguments) => {
                            // Fix malformed JSON string arguments from models
                            arguments = self.fix_json_string_arguments(arguments);
                            
                            let tool_call = crate::types::ToolCall {
                                id: partial_call.id,
                                name: partial_call.name,
                                arguments,
                            };
                            self.action_tx.send(Action::ExecuteToolCall(tool_call))?;
                        }
                        Err(e) => {
                            error!("Failed to parse tool call arguments: {}", e);
                            self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                                message: format!("Invalid tool call arguments: {}", e),
                                timestamp: chrono::Utc::now(),
                            }))?;
                        }
                    }
                }
            }
            StreamChunk::Done => {
                self.action_tx.send(Action::StreamComplete)?;
            }
        }
        
        Ok(())
    }
    
    /// Fix malformed JSON string arguments from LLM models.
    /// Some models incorrectly return argument values as JSON strings instead of their actual types.
    /// For example: {"query": "[\"value\"]"} instead of {"query": ["value"]}
    fn fix_json_string_arguments(&self, mut arguments: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = arguments.as_object_mut() {
            for (_key, value) in obj.iter_mut() {
                if let Some(string_val) = value.as_str() {
                    // Try to parse the string as JSON
                    if let Ok(parsed_val) = serde_json::from_str::<serde_json::Value>(string_val) {
                        // Only replace if the parsed value is not a string (to avoid infinite recursion)
                        match parsed_val {
                            serde_json::Value::Array(_) | 
                            serde_json::Value::Object(_) | 
                            serde_json::Value::Number(_) | 
                            serde_json::Value::Bool(_) |
                            serde_json::Value::Null => {
                                *value = parsed_val;
                            }
                            _ => {
                                // If it's still a string, don't replace
                            }
                        }
                    }
                }
            }
        }
        arguments
    }
    
    async fn handle_execute_tool_call(&mut self, tool_call: crate::types::ToolCall) -> Result<()> {
        debug!("Executing tool call: {} with args: {}", tool_call.name, tool_call.arguments);
        
        // Add tool call to chat
        self.action_tx.send(Action::AddChatMessage(ChatMessage::ToolCall { 
            name: tool_call.name.clone(),
            params: tool_call.arguments.to_string(),
            timestamp: chrono::Utc::now(),
        }))?;
        
        let mcp_client = match &self.mcp_client {
            Some(client) => client,
            None => {
                self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                    message: "MCP client not initialized".to_string(),
                    timestamp: chrono::Utc::now(),
                }))?;
                return Ok(());
            }
        };
        
        // Execute the tool via MCP with timeout
        let execution_future = mcp_client.execute_tool(&tool_call.name, tool_call.arguments);
        let timeout_duration = std::time::Duration::from_secs(30); // 30 second timeout
        
        match tokio::time::timeout(timeout_duration, execution_future).await {
            Ok(Ok(result)) => {
                let result_string = result.to_string();
                self.action_tx.send(Action::ToolExecutionResult { 
                    result: result_string 
                })?;
            }
            Ok(Err(e)) => {
                error!("MCP tool execution failed: {}", e);
                self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                    message: format!("Tool execution failed: {}", e),
                    timestamp: chrono::Utc::now(),
                }))?;
            }
            Err(_) => {
                error!("MCP tool execution timed out: {}", tool_call.name);
                self.action_tx.send(Action::AddChatMessage(ChatMessage::Error { 
                    message: format!("Tool execution timed out after {} seconds", timeout_duration.as_secs()),
                    timestamp: chrono::Utc::now(),
                }))?;
            }
        }
        
        Ok(())
    }

    async fn handle_refresh_tools(&mut self) -> Result<()> {
        debug!("Refreshing tools from MCP servers");
        
        let mcp_client = match &mut self.mcp_client {
            Some(client) => client,
            None => {
                self.action_tx.send(Action::Error("MCP client not initialized".to_string()))?;
                return Ok(());
            }
        };
        
        // Rediscover tools from all connected servers
        match mcp_client.discover_tools().await {
            Ok(()) => {
                info!("Successfully refreshed tools from MCP servers");
            }
            Err(e) => {
                error!("Failed to refresh tools: {}", e);
                self.action_tx.send(Action::Error(format!("Failed to refresh tools: {}", e)))?;
            }
        }
        
        Ok(())
    }
    
}


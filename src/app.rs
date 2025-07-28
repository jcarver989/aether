use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

use crate::{
    action::Action,
    agent::Agent,
    cli::Cli,
    components::{Component, fps::FpsCounter, home::Home},
    config::{AppConfig, Config},
    llm::LlmProvider,
    tui::{Event, Tui},
    types::ChatMessage,
};

pub struct App<T: LlmProvider> {
    config: Arc<Config>,
    app_config: AppConfig,
    components: Vec<Box<dyn Component>>,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,
    agent: Agent<T>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Home,
}

impl<T: LlmProvider> App<T> {
    pub fn new(cli_args: &Cli, agent: Agent<T>) -> Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();

        // Load configuration with CLI args
        let config = Config::with_cli_args(Some(cli_args))?;
        let app_config = config.config.clone();
        let config = Arc::new(config);

        Ok(Self {
            components: vec![Box::new(Home::new()), Box::new(FpsCounter::default())],
            should_quit: false,
            should_suspend: false,
            config,
            app_config,
            mode: Mode::Home,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
            agent,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut tui = Tui::new()?
            .mouse(true) // Enable mouse support for scrolling
            .tick_rate(self.app_config.ui.tick_rate)
            .frame_rate(self.app_config.ui.frame_rate);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(self.action_tx.clone())?;
        }
        for component in self.components.iter_mut() {
            component.register_config_handler(Arc::clone(&self.config))?;
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
                // Mouse support already enabled during tui initialization
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
            Event::Key(key) => {
                // First let components handle the key event
                let mut key_handled = false;
                for component in self.components.iter_mut() {
                    if let Some(action) = component.handle_key_event(key)? {
                        action_tx.send(action)?;
                        key_handled = true;
                        break; // Stop after first component handles the key
                    }
                }
                // Only check global keybindings if no component handled the key
                if !key_handled {
                    self.handle_key_event(key)?;
                }
            }
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
                    self.handle_submit_message(message).await?;
                }
                Action::ReceiveStreamChunk(ref chunk) => {
                    self.handle_stream_chunk(chunk).await?;
                }
                Action::StreamContent(ref content) => {
                    // Update or create AssistantStreaming message in agent
                    self.agent.append_streaming_content(content);
                }
                Action::StartStreaming => {
                    // Create initial empty streaming message in agent
                    self.agent.add_message(ChatMessage::AssistantStreaming {
                        content: String::new(),
                        timestamp: chrono::Utc::now(),
                    });
                }
                Action::ExecuteToolCall(ref tool_call) => {
                    self.handle_execute_tool_call(tool_call).await?;
                }
                Action::ReceiveAssistantMessage(ref message) => {
                    let assistant_message = ChatMessage::Assistant {
                        content: message.to_string(),
                        timestamp: chrono::Utc::now(),
                    };
                    self.action_tx
                        .send(Action::AddChatMessage(assistant_message.clone()))?;
                    self.agent.add_message(assistant_message);
                }
                Action::ToolExecutionResult {
                    ref tool_call_id,
                    ref result,
                } => {
                    let tool_result_message = ChatMessage::ToolResult {
                        tool_call_id: tool_call_id.to_string(),
                        content: result.to_string(),
                        timestamp: chrono::Utc::now(),
                    };
                    self.action_tx
                        .send(Action::AddChatMessage(tool_result_message.clone()))?;
                    self.agent.add_message(tool_result_message);
                    // Force a render to ensure tool results are displayed immediately
                    self.action_tx.send(Action::Render)?;
                    // Automatically continue the conversation after tool execution
                    self.action_tx.send(Action::ContinueConversation)?;
                }
                Action::RefreshTools => {
                    // Tool refresh not implemented - tools are loaded at startup
                }
                Action::AddChatMessage(ref _message) => {
                    // Messages are already added to agent in their respective handlers
                    // This is now just for UI components to receive the message
                }
                Action::ClearChat => {
                    self.agent.clear_history();
                }
                Action::ContinueConversation => {
                    self.handle_continue_conversation().await?;
                }
                Action::StreamComplete => {
                    // Convert the last AssistantStreaming message to Assistant message
                    self.agent.finalize_streaming_message();
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
                        .send(Action::Error(format!("Failed to draw: {err:?}")));
                }
            }
        })?;
        Ok(())
    }

    async fn handle_submit_message(&mut self, user_input: &str) -> Result<()> {
        debug!("Handling user message: {}", user_input);

        // Add user message to both UI and agent
        let user_message = ChatMessage::User {
            content: user_input.to_string(),
            timestamp: chrono::Utc::now(),
        };

        self.action_tx
            .send(Action::AddChatMessage(user_message.clone()))?;

        self.agent.add_message(user_message);

        // Send to LLM with the user input
        self.send_to_llm(Some(user_input.to_string())).await
    }

    async fn handle_stream_chunk(
        &mut self,
        chunk: &crate::llm::provider::StreamChunk,
    ) -> Result<()> {
        use crate::agent::PartialToolCall;
        use crate::llm::provider::StreamChunk;

        match chunk {
            StreamChunk::Content(content) => {
                self.action_tx.send(Action::StreamContent(content.clone()))?;
            }
            StreamChunk::ToolCallStart { id, name } => {
                // Start tracking this tool call in the agent
                self.agent.active_tool_calls_mut().insert(
                    id.clone(),
                    PartialToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: String::new(),
                    },
                );

                self.action_tx.send(Action::StreamToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: String::new(),
                })?;
            }
            StreamChunk::ToolCallArgument { id, argument } => {
                // Accumulate arguments for this tool call
                let mut tool_call_info = None;
                if let Some(partial_call) = self.agent.active_tool_calls_mut().get_mut(id) {
                    partial_call.arguments.push_str(argument);
                    tool_call_info =
                        Some((partial_call.name.clone(), partial_call.arguments.clone()));
                }

                if let Some((name, arguments)) = tool_call_info {
                    // Update the UI with the accumulated arguments
                    self.action_tx.send(Action::StreamToolCall {
                        id: id.clone(),
                        name,
                        arguments,
                    })?;
                }
            }
            StreamChunk::ToolCallComplete { id } => {
                // Tool call is complete, execute it
                let partial_call = self.agent.active_tool_calls_mut().remove(id);

                if let Some(partial_call) = partial_call {
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
                            self.action_tx
                                .send(Action::AddChatMessage(ChatMessage::Error {
                                    message: format!("Invalid tool call arguments: {e}"),
                                    timestamp: chrono::Utc::now(),
                                }))?;
                            self.action_tx.send(Action::Render)?;
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
                            serde_json::Value::Array(_)
                            | serde_json::Value::Object(_)
                            | serde_json::Value::Number(_)
                            | serde_json::Value::Bool(_)
                            | serde_json::Value::Null => {
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

    async fn handle_execute_tool_call(&mut self, tool_call: &crate::types::ToolCall) -> Result<()> {
        debug!(
            "Executing tool call: {} with args: {}",
            tool_call.name, tool_call.arguments
        );

        // Check for potential loop using agent's loop detection
        let now = chrono::Utc::now();
        let duplicate_count = self
            .agent
            .check_tool_loop(&tool_call.name, &tool_call.arguments, 2);

        if duplicate_count >= 3 {
            warn!(
                "Potential loop detected: tool '{}' called {} times with same arguments recently",
                tool_call.name, duplicate_count
            );
            // Send loop detection error as tool result
            self.action_tx.send(Action::ToolExecutionResult {
                tool_call_id: tool_call.id.clone(),
                result: format!(
                    "Error: Loop detected - tool '{}' has been called {} times recently with the same arguments. Please try a different approach or provide different parameters.",
                    tool_call.name, duplicate_count + 1
                ),
            })?;
            return Ok(());
        }

        // Record this tool call in the agent
        self.agent
            .record_tool_call(tool_call.name.clone(), tool_call.arguments.clone(), now);

        // Add tool call to both UI and agent
        let tool_call_message = ChatMessage::ToolCall {
            id: tool_call.id.clone(),
            name: tool_call.name.clone(),
            params: tool_call.arguments.to_string(),
            timestamp: chrono::Utc::now(),
        };

        self.action_tx
            .send(Action::AddChatMessage(tool_call_message.clone()))?;
        self.action_tx.send(Action::Render)?;

        self.agent.add_message(tool_call_message);

        // Execute the tool using the agent
        let result = self
            .agent
            .execute_tool(&tool_call.name, tool_call.arguments.clone())
            .await;

        let result_content = match result {
            Ok(result_value) => {
                // Convert the result to a readable string
                match result_value {
                    serde_json::Value::String(s) => s,
                    other => serde_json::to_string_pretty(&other)
                        .unwrap_or_else(|_| format!("{other:?}")),
                }
            }
            Err(e) => {
                format!("Tool execution failed: {e}")
            }
        };

        self.action_tx.send(Action::ToolExecutionResult {
            tool_call_id: tool_call.id.clone(),
            result: result_content,
        })?;

        Ok(())
    }

    async fn handle_continue_conversation(&mut self) -> Result<()> {
        debug!("Continuing conversation after tool execution");
        // Simply send the current conversation state to LLM (no new user message)
        self.send_to_llm(None).await
    }

    /// Send conversation to LLM with optional additional user message
    async fn send_to_llm(&mut self, additional_user_message: Option<String>) -> Result<()> {
        // Start streaming
        self.action_tx.send(Action::StartStreaming)?;

        // Add additional user message if provided
        if let Some(user_input) = additional_user_message {
            let user_message = ChatMessage::User {
                content: user_input,
                timestamp: chrono::Utc::now(),
            };
            self.agent.add_message(user_message);
        }

        // Get the stream from the agent
        let stream = self.agent.stream_completion(Some(0.7)).await?;

        let tx_clone = self.action_tx.clone();

        // Spawn background task to handle stream
        tokio::spawn(async move {
            let mut stream = stream;
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
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

        Ok(())
    }
}

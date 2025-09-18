use crate::{colors, ui};
use crossterm::style::Stylize;
use std::collections::HashMap;
use std::io::{Write, stdout};

#[derive(Debug)]
pub struct PartialToolCall {
    pub name: String,
    pub model_name: String,
    pub arguments: String,
    pub progress_bar: Option<ui::CrosstermSpinner>,
}

#[derive(Debug)]
pub struct AppView {
    active_tool_calls: HashMap<String, PartialToolCall>,
    message_started: bool,
}

impl AppView {
    pub fn new() -> Self {
        Self {
            active_tool_calls: HashMap::new(),
            message_started: false,
        }
    }

    pub fn stop_all_spinners(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for (_, mut tool_call) in self.active_tool_calls.drain() {
            if let Some(ref mut spinner) = tool_call.progress_bar {
                spinner.finish_and_clear()?;
            }
        }
        Ok(())
    }

    pub fn update(
        &mut self,
        event: aether::agent::AgentMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use aether::agent::AgentMessage::*;

        match event {
            Text {
                chunk,
                is_complete,
                model_name,
                ..
            } => {
                if is_complete {
                    self.message_started = false;
                    self.render_text_complete()?;
                } else {
                    if let Some(filtered_chunk) = ui::filter_text_chunk(&chunk) {
                        let is_first_chunk = !self.message_started;
                        self.message_started = true;
                        self.render_text_chunk(filtered_chunk, model_name, is_first_chunk)?;
                    }
                }
            }

            ToolCall {
                tool_call_id,
                name,
                arguments,
                model_name,
                is_complete: false,
                ..
            } => {
                use std::collections::hash_map::Entry;

                match self.active_tool_calls.entry(tool_call_id.clone()) {
                    Entry::Occupied(mut entry) => {
                        if let Some(args_chunk) = arguments.as_ref() {
                            entry.get_mut().arguments.push_str(args_chunk);
                        }
                    }
                    Entry::Vacant(entry) => {
                        let mut spinner = ui::create_tool_spinner(&name, &model_name)?;
                        spinner.start()?;
                        entry.insert(PartialToolCall {
                            name: name.clone(),
                            model_name: model_name.clone(),
                            arguments: arguments.clone().unwrap_or_default(),
                            progress_bar: Some(spinner),
                        });
                    }
                }
            }

            ToolCall {
                tool_call_id,
                result,
                is_complete: true,
                ..
            } => {
                if let Some(mut tool_call) = self.active_tool_calls.remove(&tool_call_id) {
                    // Ensure spinner is stopped immediately
                    if let Some(ref mut spinner) = tool_call.progress_bar {
                        spinner.finish_and_clear()?;
                    }

                    let args_to_show = if tool_call.arguments.is_empty() {
                        None
                    } else {
                        Some(tool_call.arguments)
                    };

                    self.render_tool_call_complete(
                        &tool_call.name,
                        &tool_call.model_name,
                        args_to_show,
                        result,
                        tool_call.progress_bar,
                    )?;
                }
            }

            Error { message } => {
                self.render_error(&message)?;
            }

            Cancelled { message } => {
                self.render_cancelled(&message)?;
            }

            ElicitationRequest {
                request,
                response_sender,
                ..
            } => {
                self.render_elicitation_request(request, response_sender)?;
            }

            Done => {
                self.render_done()?;
            }
        }

        Ok(())
    }

    fn render_done(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Just mark completion - main loop will handle returning to user input
        Ok(())
    }

    fn render_text_chunk(
        &self,
        content: String,
        model_name: String,
        is_first_chunk: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut stdout = stdout();

        if is_first_chunk {
            println!(
                "{} {} ",
                "◈".with(colors::primary()).bold(),
                format!("({})", ui::format_model_name(&model_name))
                    .with(colors::text_secondary())
                    .dim()
            );
        }

        if !content.trim().is_empty() {
            print!("{}", content.with(colors::text_primary()));
            stdout.flush()?;
        }

        Ok(())
    }

    fn render_text_complete(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n");
        stdout().flush()?;
        Ok(())
    }

    fn render_tool_call_complete(
        &self,
        name: &str,
        model_name: &str,
        arguments: Option<String>,
        result: Option<String>,
        progress_bar: Option<ui::CrosstermSpinner>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut progress_bar) = progress_bar {
            progress_bar.finish_and_clear()?;
        }

        // Show tool completion
        println!(
            "{} {} {} {}",
            "✓".with(colors::success()).bold(),
            format!("({})", ui::format_model_name(model_name))
                .with(colors::text_secondary())
                .dim(),
            "Tool".bold().with(colors::text_primary()),
            name.bold().with(colors::success())
        );

        // Show tool details
        ui::show_tool_details(arguments.as_deref(), result.as_deref())?;

        // Ensure output is flushed
        stdout().flush()?;

        Ok(())
    }

    fn render_error(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "{} {}",
            "✗".with(colors::error()).bold(),
            message.with(colors::error())
        );
        Ok(())
    }

    fn render_cancelled(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "{} {}",
            "⊘".with(colors::warning()).bold(),
            message.with(colors::warning())
        );
        Ok(())
    }

    fn render_elicitation_request(
        &self,
        request: aether::CreateElicitationRequestParam,
        response_sender: tokio::sync::oneshot::Sender<aether::CreateElicitationResult>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "\n{}",
            "🤖 AI Request for Permission"
                .with(crate::colors::primary())
                .bold()
        );
        println!("{}", request.message.with(crate::colors::text_primary()));

        use aether::{CreateElicitationResult, ElicitationAction};
        use inquire::Confirm;

        let confirm_result = Confirm::new("Do you want to allow this action?")
            .with_default(false)
            .with_help_message("The AI is requesting permission to proceed")
            .prompt();

        let result = match confirm_result {
            Ok(true) => CreateElicitationResult {
                action: ElicitationAction::Accept,
                content: None,
            },
            Ok(false) => CreateElicitationResult {
                action: ElicitationAction::Decline,
                content: None,
            },
            Err(_) => CreateElicitationResult {
                action: ElicitationAction::Cancel,
                content: None,
            },
        };

        let _ = response_sender.send(result);
        println!();

        Ok(())
    }

    pub fn reset_for_new_conversation(&mut self) {
        self.message_started = false;
        // Keep active_tool_calls in case there are any lingering ones
    }
}

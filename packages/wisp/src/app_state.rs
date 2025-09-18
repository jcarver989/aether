use crate::{PartialToolCall, ui, ui_event::UiEvent};
use std::collections::HashMap;

#[derive(Debug)]
pub struct AppState {
    active_tool_calls: HashMap<String, PartialToolCall>,
    message_started: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_tool_calls: HashMap::new(),
            message_started: false,
        }
    }

    pub fn update(
        &mut self,
        event: aether::agent::AgentMessage,
    ) -> Result<Vec<UiEvent>, Box<dyn std::error::Error>> {
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
                    Ok(vec![UiEvent::TextComplete])
                } else {
                    if let Some(filtered_chunk) = ui::filter_text_chunk(&chunk) {
                        let is_first_chunk = !self.message_started;
                        self.message_started = true;
                        Ok(vec![UiEvent::TextChunk {
                            content: filtered_chunk,
                            model_name,
                            is_first_chunk,
                        }])
                    } else {
                        Ok(vec![])
                    }
                }
            }

            ToolCall {
                tool_call_id,
                name,
                arguments,
                result,
                is_complete,
                model_name,
            } => {
                tracing::debug!("AppState received ToolCall: id={}, name={}, is_complete={}, has_result={}",
                    tool_call_id, name, is_complete, result.is_some());
                if is_complete {
                    if let Some(tool_call) = self.active_tool_calls.remove(&tool_call_id) {
                        // Clean up the progress bar
                        tool_call.progress_bar.finish_and_clear();

                        let args_to_show = if tool_call.arguments.is_empty() {
                            None
                        } else {
                            Some(tool_call.arguments)
                        };

                        Ok(vec![UiEvent::ToolCompleted {
                            name: tool_call.name,
                            model_name: tool_call.model_name,
                            arguments: args_to_show,
                            result,
                        }])
                    } else {
                        Ok(vec![])
                    }
                } else if !name.is_empty() {
                    // Tool starting - create spinner and initialize arguments
                    let pb = ui::create_tool_spinner(&name, &model_name)?;
                    self.active_tool_calls.insert(
                        tool_call_id.clone(),
                        PartialToolCall {
                            name: name.clone(),
                            model_name: model_name.clone(),
                            arguments: String::new(),
                            progress_bar: pb,
                        },
                    );

                    Ok(vec![UiEvent::ToolStarted {
                        id: tool_call_id,
                        name,
                        model_name,
                    }])
                } else if let Some(args_chunk) = arguments {
                    // Tool argument chunk - accumulate arguments
                    if let Some(tool_call) = self.active_tool_calls.get_mut(&tool_call_id) {
                        tool_call.arguments.push_str(&args_chunk);
                    }
                    Ok(vec![])
                } else {
                    Ok(vec![])
                }
            }

            Error { message } => Ok(vec![UiEvent::Error { message }]),

            Cancelled { message } => Ok(vec![UiEvent::Cancelled { message }]),

            ElicitationRequest {
                request,
                response_sender,
                ..
            } => Ok(vec![UiEvent::ElicitationRequest {
                request,
                response_sender,
            }]),
        }
    }
}

use crate::components::command_picker::{CommandEntry, CommandPicker, CommandPickerMessage};
use crate::components::file_picker::{FilePicker, FilePickerMessage};
use crate::components::input_prompt::InputPrompt;
use crate::components::text_input::{SelectedFileMention, TextInput, TextInputMessage};
use crate::keybindings::Keybindings;
use crate::tui::KeyCode;
use crate::tui::{Component, Cursor, Event, Line, PickerMessage, ViewContext};
use std::collections::HashSet;

use super::app::PromptAttachment;

#[derive(Debug)]
pub enum PromptComposerMessage {
    SubmitRequested {
        user_input: String,
        attachments: Vec<PromptAttachment>,
    },
    OpenConfig,
    OpenSessionPicker,
    ClearScreen,
}

pub struct PromptComposer {
    text_input: TextInput,
    available_commands: Vec<CommandEntry>,
    file_picker: Option<FilePicker>,
    command_picker: Option<CommandPicker>,
}

impl Default for PromptComposer {
    fn default() -> Self {
        Self::new(Keybindings::default())
    }
}

impl PromptComposer {
    pub fn new(keybindings: Keybindings) -> Self {
        Self {
            text_input: TextInput::new(keybindings),
            available_commands: Vec::new(),
            file_picker: None,
            command_picker: None,
        }
    }

    pub fn set_available_commands(&mut self, commands: Vec<CommandEntry>) {
        self.available_commands = commands;
    }

    #[cfg(test)]
    pub fn has_active_picker(&self) -> bool {
        self.file_picker.is_some() || self.command_picker.is_some()
    }

    #[cfg(test)]
    pub(crate) fn set_input(&mut self, input: String) {
        self.text_input.set_input(input);
    }

    #[cfg(test)]
    pub(crate) fn buffer(&self) -> &str {
        self.text_input.buffer()
    }

    #[cfg(test)]
    pub(crate) fn cursor_index(&self) -> usize {
        let picker_query_len = self.file_picker.as_ref().map(|picker| picker.query().len());
        self.text_input.cursor_index(picker_query_len)
    }

    #[cfg(test)]
    pub(crate) fn apply_file_selection(&mut self, path: std::path::PathBuf, display_name: String) {
        self.text_input.apply_file_selection(path, display_name);
    }

    #[cfg(test)]
    pub(crate) fn open_command_picker_with_entries(&mut self, commands: Vec<CommandEntry>) {
        self.command_picker = Some(CommandPicker::new(commands));
    }

    pub fn close_all(&mut self) {
        self.file_picker = None;
        self.command_picker = None;
    }

    pub fn cursor(&self, context: &ViewContext) -> Cursor {
        let picker_query_len = self.file_picker.as_ref().map(|picker| picker.query().len());
        let layout = InputPrompt {
            input: self.text_input.buffer(),
            cursor_index: self.text_input.cursor_index(picker_query_len),
        }
        .layout(context);

        Cursor {
            row: layout.cursor_row,
            col: layout.cursor_col as usize,
            is_visible: true,
        }
    }

    #[cfg(test)]
    pub(crate) fn has_file_picker(&self) -> bool {
        self.file_picker.is_some()
    }

    #[cfg(test)]
    pub(crate) fn has_command_picker(&self) -> bool {
        self.command_picker.is_some()
    }

    fn handle_picker_outcome<T>(
        &mut self,
        outcome: Option<Vec<PickerMessage<T>>>,
    ) -> (bool, Option<T>) {
        let Some(msg) = outcome.unwrap_or_default().into_iter().next() else {
            return (false, None);
        };
        match msg {
            PickerMessage::Close => (true, None),
            PickerMessage::CloseAndPopChar => {
                self.text_input.delete_char_before_cursor();
                (true, None)
            }
            PickerMessage::CloseWithChar(c) => {
                self.text_input.insert_char_at_cursor(c);
                (true, None)
            }
            PickerMessage::Confirm(value) => (true, Some(value)),
            PickerMessage::CharTyped(c) => {
                self.text_input.insert_char_at_cursor(c);
                (false, None)
            }
            PickerMessage::PopChar => {
                self.text_input.delete_char_before_cursor();
                (false, None)
            }
        }
    }

    fn handle_file_picker_outcome(
        &mut self,
        outcome: Option<Vec<FilePickerMessage>>,
    ) -> Vec<PromptComposerMessage> {
        let (close, confirmed) = self.handle_picker_outcome(outcome);
        if let Some(file_match) = confirmed {
            self.file_picker = None;
            self.text_input
                .apply_file_selection(file_match.path, file_match.display_name);
        } else if close {
            self.file_picker = None;
        }
        vec![]
    }

    fn handle_command_picker_outcome(
        &mut self,
        outcome: Option<Vec<CommandPickerMessage>>,
    ) -> Vec<PromptComposerMessage> {
        let (close, confirmed) = self.handle_picker_outcome(outcome);
        if let Some(cmd) = confirmed {
            self.command_picker = None;
            return self.apply_command(&cmd);
        } else if close {
            self.command_picker = None;
        }
        vec![]
    }

    fn handle_text_input_outcome(
        &mut self,
        outcome: Option<Vec<TextInputMessage>>,
    ) -> Option<Vec<PromptComposerMessage>> {
        let msgs = outcome?;
        match msgs.into_iter().next() {
            Some(TextInputMessage::Submit) => Some(self.prepare_submit()),
            Some(TextInputMessage::OpenCommandPicker) => {
                let mut commands = builtin_commands();
                commands.extend(self.available_commands.clone());
                self.command_picker = Some(CommandPicker::new(commands));
                Some(vec![])
            }
            Some(TextInputMessage::OpenFilePicker) => {
                self.file_picker = Some(FilePicker::new());
                Some(vec![])
            }
            None => Some(vec![]),
        }
    }

    fn apply_command(&mut self, cmd: &CommandEntry) -> Vec<PromptComposerMessage> {
        if cmd.builtin && cmd.name == "clear" {
            self.text_input.clear();
            self.close_all();
            vec![PromptComposerMessage::ClearScreen]
        } else if cmd.builtin && cmd.name == "config" {
            self.text_input.clear();
            self.close_all();
            vec![PromptComposerMessage::OpenConfig]
        } else if cmd.builtin && cmd.name == "resume" {
            self.text_input.clear();
            self.close_all();
            vec![PromptComposerMessage::OpenSessionPicker]
        } else if cmd.has_input {
            self.text_input.set_input(format!("/{} ", cmd.name));
            vec![]
        } else {
            self.text_input.set_input(format!("/{}", cmd.name));
            self.prepare_submit()
        }
    }

    fn prepare_submit(&mut self) -> Vec<PromptComposerMessage> {
        if self.text_input.buffer().trim().is_empty() {
            return vec![];
        }

        let user_input = self.text_input.buffer().trim().to_string();
        let attachments = collect_submit_attachments(&user_input, self.text_input.take_mentions());
        self.text_input.clear();
        self.close_all();

        vec![PromptComposerMessage::SubmitRequested {
            user_input,
            attachments,
        }]
    }
}

impl Component for PromptComposer {
    type Message = PromptComposerMessage;

    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        match event {
            Event::Paste(text) => {
                self.close_all();
                self.text_input.insert_paste(text);
                Some(vec![])
            }
            Event::Key(key_event) => {
                if let Some(ref mut picker) = self.file_picker {
                    let outcome = picker.on_event(event);
                    if outcome.is_some() {
                        return Some(self.handle_file_picker_outcome(outcome));
                    }

                    if matches!(
                        key_event.code,
                        KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
                    ) {
                        return Some(vec![]);
                    }
                }

                if let Some(ref mut picker) = self.command_picker {
                    let outcome = picker.on_event(event);
                    return Some(self.handle_command_picker_outcome(outcome));
                }

                let outcome = self.text_input.on_event(event);
                self.handle_text_input_outcome(outcome)
            }
            _ => None,
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        let picker_query_len = self.file_picker.as_ref().map(|picker| picker.query().len());
        let mut lines = InputPrompt {
            input: self.text_input.buffer(),
            cursor_index: self.text_input.cursor_index(picker_query_len),
        }
        .layout(context)
        .lines;

        if let Some(ref picker) = self.file_picker {
            lines.extend(picker.render(context));
        }

        if let Some(ref picker) = self.command_picker {
            lines.extend(picker.render(context));
        }

        lines
    }
}

fn collect_submit_attachments(
    user_input: &str,
    selected_mentions: Vec<SelectedFileMention>,
) -> Vec<PromptAttachment> {
    let mentions: HashSet<&str> = user_input.split_whitespace().collect();
    selected_mentions
        .into_iter()
        .filter(|mention| mentions.contains(mention.mention.as_str()))
        .map(|mention| PromptAttachment {
            path: mention.path,
            display_name: mention.display_name,
        })
        .collect()
}

fn builtin_commands() -> Vec<CommandEntry> {
    vec![
        CommandEntry {
            name: "clear".into(),
            description: "Clear the screen".into(),
            has_input: false,
            hint: None,
            builtin: true,
        },
        CommandEntry {
            name: "config".into(),
            description: "Open configuration settings".into(),
            has_input: false,
            hint: None,
            builtin: true,
        },
        CommandEntry {
            name: "resume".into(),
            description: "Resume a previous session".into(),
            has_input: false,
            hint: None,
            builtin: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn builtin_config_command_emits_open_config() {
        let mut composer = PromptComposer::default();

        composer.on_event(&key(KeyCode::Char('/')));

        assert!(composer.has_command_picker());

        // Navigate past "clear" to "config"
        composer.on_event(&key(KeyCode::Down));

        let outcome = composer.on_event(&key(KeyCode::Enter));
        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PromptComposerMessage::OpenConfig]
        ));
        assert_eq!(composer.buffer(), "");
        assert!(!composer.has_active_picker());
    }

    #[test]
    fn command_without_input_requests_submit_immediately() {
        let mut composer = PromptComposer::default();
        composer.set_available_commands(vec![CommandEntry {
            name: "status".into(),
            description: "status".into(),
            has_input: false,
            hint: None,
            builtin: false,
        }]);

        composer.on_event(&key(KeyCode::Char('/')));
        composer.on_event(&key(KeyCode::Char('s')));

        let outcome = composer.on_event(&key(KeyCode::Enter));
        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PromptComposerMessage::SubmitRequested { user_input, .. }]
            if user_input == "/status"
        ));
        assert_eq!(composer.buffer(), "");
    }

    #[test]
    fn command_with_input_populates_prompt_without_submit() {
        let mut composer = PromptComposer::default();
        composer.set_available_commands(vec![CommandEntry {
            name: "search".into(),
            description: "Search code".into(),
            has_input: true,
            hint: Some("query".into()),
            builtin: false,
        }]);

        composer.on_event(&key(KeyCode::Char('/')));
        composer.on_event(&key(KeyCode::Char('s')));
        let outcome = composer.on_event(&key(KeyCode::Enter));

        assert!(outcome.unwrap().is_empty());

        assert_eq!(composer.buffer(), "/search ");
        assert!(!composer.has_active_picker());
    }

    #[test]
    fn submit_filters_unmentioned_attachments() {
        let mut composer = PromptComposer::default();
        composer.set_input("inspect @keep.rs now".to_string());
        composer.apply_file_selection(PathBuf::from("/tmp/keep.rs"), "keep.rs".to_string());
        composer.set_input("inspect @keep.rs now @skip.rs".to_string());
        composer.apply_file_selection(PathBuf::from("/tmp/skip.rs"), "skip.rs".to_string());
        composer.set_input("inspect @keep.rs now".to_string());

        let messages = composer.prepare_submit();
        let [PromptComposerMessage::SubmitRequested { attachments, .. }] = messages.as_slice()
        else {
            panic!("expected submit request");
        };

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].display_name, "keep.rs");
        assert_eq!(attachments[0].path, PathBuf::from("/tmp/keep.rs"));
    }

    #[test]
    fn paste_closes_picker_and_inserts_text() {
        let mut composer = PromptComposer::default();
        composer.on_event(&key(KeyCode::Char('@')));
        assert!(composer.has_file_picker());

        let outcome = composer.on_event(&Event::Paste("pasted text".to_string()));
        assert!(outcome.is_some());
        assert!(outcome.unwrap().is_empty());
        assert!(!composer.has_active_picker());
        assert_eq!(composer.buffer(), "@pasted text");
    }

    #[test]
    fn file_picker_cursor_tracks_query_length() {
        let mut composer = PromptComposer::default();
        composer.on_event(&key(KeyCode::Char('@')));
        composer.on_event(&key(KeyCode::Char('f')));
        composer.on_event(&key(KeyCode::Char('o')));

        assert_eq!(composer.cursor_index(), 3);
    }

    #[test]
    fn command_picker_cursor_stays_in_prompt_row() {
        let mut composer = PromptComposer::default();
        composer.open_command_picker_with_entries(vec![CommandEntry {
            name: "config".into(),
            description: "Open config".into(),
            has_input: false,
            hint: None,
            builtin: true,
        }]);

        let context = ViewContext::new((120, 40));
        let output = composer.render(&context);
        let cursor = composer.cursor(&context);
        let input_row = output
            .iter()
            .position(|line| line.plain_text().contains("> "))
            .expect("input prompt should exist");

        assert_eq!(cursor.row, input_row);
    }
}

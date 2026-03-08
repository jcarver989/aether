use crate::components::command_picker::{CommandEntry, CommandPicker, CommandPickerAction};
use crate::components::file_picker::{FileMatch, FilePicker, FilePickerAction};
use crate::components::input_prompt::InputPrompt;
use crate::components::text_input::{SelectedFileMention, TextInput, TextInputAction};
use crate::tui::{Component, Cursor, InputOutcome, InteractiveComponent, Line, RenderContext};
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;

use super::app::PromptAttachment;

#[derive(Debug)]
pub enum PromptComposerAction {
    SubmitRequested {
        user_input: String,
        attachments: Vec<PromptAttachment>,
    },
    OpenConfig,
}

pub struct PromptComposer {
    text_input: TextInput,
    available_commands: Vec<CommandEntry>,
    file_picker: Option<FilePicker>,
    command_picker: Option<CommandPicker>,
}

impl Default for PromptComposer {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptComposer {
    pub fn new() -> Self {
        Self {
            text_input: TextInput::new(),
            available_commands: Vec::new(),
            file_picker: None,
            command_picker: None,
        }
    }

    pub fn set_available_commands(&mut self, commands: Vec<CommandEntry>) {
        self.available_commands = commands;
    }

    pub fn on_paste(&mut self, text: &str) -> bool {
        self.close_all();
        self.text_input.insert_paste(text);
        true
    }

    #[allow(dead_code)]
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

    pub fn cursor(&self, context: &RenderContext) -> Cursor {
        let picker_query_len = self.file_picker.as_ref().map(|picker| picker.query().len());
        let layout = InputPrompt {
            input: self.text_input.buffer(),
            cursor_index: self.text_input.cursor_index(picker_query_len),
        }
        .layout(context);

        Cursor {
            logical_row: layout.cursor_row,
            col: layout.cursor_col as usize,
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

    #[cfg(test)]
    pub(crate) fn available_commands(&self) -> &[CommandEntry] {
        &self.available_commands
    }

    pub(crate) fn open_file_picker_with_matches(&mut self, matches: Vec<FileMatch>) {
        self.file_picker = Some(FilePicker::from_matches(matches));
    }

    fn handle_file_picker_outcome(
        &mut self,
        outcome: &InputOutcome<FilePickerAction>,
    ) -> InputOutcome<PromptComposerAction> {
        match outcome.action {
            Some(FilePickerAction::Close) => {
                self.file_picker = None;
            }
            Some(FilePickerAction::CloseAndPopChar) => {
                self.text_input.delete_char_before_cursor();
                self.file_picker = None;
            }
            Some(FilePickerAction::CloseWithChar(c)) => {
                self.text_input.insert_char_at_cursor(c);
                self.file_picker = None;
            }
            Some(FilePickerAction::ConfirmSelection) => {
                let selected = self
                    .file_picker
                    .take()
                    .and_then(|picker| picker.selected().cloned());
                if let Some(selected) = selected {
                    self.text_input
                        .apply_file_selection(selected.path, selected.display_name);
                }
            }
            Some(FilePickerAction::CharTyped(c)) => {
                self.text_input.insert_char_at_cursor(c);
            }
            Some(FilePickerAction::PopChar) => {
                self.text_input.delete_char_before_cursor();
            }
            None => {}
        }

        InputOutcome {
            consumed: outcome.consumed,
            needs_render: outcome.needs_render,
            action: None,
        }
    }

    fn handle_command_picker_outcome(
        &mut self,
        outcome: &InputOutcome<CommandPickerAction>,
    ) -> InputOutcome<PromptComposerAction> {
        match outcome.action {
            Some(CommandPickerAction::Close) => {
                self.command_picker = None;
            }
            Some(CommandPickerAction::CloseAndPopChar) => {
                self.text_input.delete_char_before_cursor();
                self.command_picker = None;
            }
            Some(CommandPickerAction::CloseWithChar(c)) => {
                self.text_input.insert_char_at_cursor(c);
                self.command_picker = None;
            }
            Some(CommandPickerAction::CommandChosen(ref cmd)) => {
                self.command_picker = None;
                return self.apply_command(cmd);
            }
            Some(CommandPickerAction::CharTyped(c)) => {
                self.text_input.insert_char_at_cursor(c);
            }
            Some(CommandPickerAction::PopChar) => {
                self.text_input.delete_char_before_cursor();
            }
            None => {}
        }

        InputOutcome {
            consumed: outcome.consumed,
            needs_render: outcome.needs_render,
            action: None,
        }
    }

    fn handle_text_input_outcome(
        &mut self,
        outcome: &InputOutcome<TextInputAction>,
    ) -> InputOutcome<PromptComposerAction> {
        match outcome.action {
            Some(TextInputAction::Submit) => self.prepare_submit(),
            Some(TextInputAction::OpenCommandPicker) => {
                let mut commands = builtin_commands();
                commands.extend(self.available_commands.clone());
                self.command_picker = Some(CommandPicker::new(commands));
                InputOutcome::consumed_and_render()
            }
            Some(TextInputAction::OpenFilePicker) => {
                self.file_picker = Some(FilePicker::new());
                InputOutcome::consumed_and_render()
            }
            None => InputOutcome {
                consumed: outcome.consumed,
                needs_render: outcome.needs_render,
                action: None,
            },
        }
    }

    fn apply_command(&mut self, cmd: &CommandEntry) -> InputOutcome<PromptComposerAction> {
        if cmd.builtin && cmd.name == "config" {
            self.text_input.clear();
            self.close_all();
            InputOutcome::action_and_render(PromptComposerAction::OpenConfig)
        } else if cmd.has_input {
            self.text_input.set_input(format!("/{} ", cmd.name));
            InputOutcome::consumed_and_render()
        } else {
            self.text_input.set_input(format!("/{}", cmd.name));
            self.prepare_submit()
        }
    }

    fn prepare_submit(&mut self) -> InputOutcome<PromptComposerAction> {
        if self.text_input.buffer().trim().is_empty() {
            return InputOutcome::consumed_and_render();
        }

        let user_input = self.text_input.buffer().trim().to_string();
        let attachments = collect_submit_attachments(&user_input, self.text_input.take_mentions());
        self.text_input.clear();
        self.close_all();

        InputOutcome::action_and_render(PromptComposerAction::SubmitRequested {
            user_input,
            attachments,
        })
    }
}

impl InteractiveComponent for PromptComposer {
    type Action = PromptComposerAction;

    fn on_key_event(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        if let Some(ref mut picker) = self.file_picker {
            let outcome = picker.on_key_event(key_event);
            if outcome.consumed {
                return self.handle_file_picker_outcome(&outcome);
            }

            if matches!(
                key_event.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
            ) {
                return InputOutcome::consumed();
            }
        }

        if let Some(ref mut picker) = self.command_picker {
            let outcome = picker.on_key_event(key_event);
            return self.handle_command_picker_outcome(&outcome);
        }

        let outcome = self.text_input.on_key_event(key_event);
        self.handle_text_input_outcome(&outcome)
    }
}

impl Component for PromptComposer {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
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
    vec![CommandEntry {
        name: "config".into(),
        description: "Open configuration settings".into(),
        has_input: false,
        hint: None,
        builtin: true,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn builtin_config_command_emits_open_config() {
        let mut composer = PromptComposer::new();

        let outcome = composer.on_key_event(key(KeyCode::Char('/')));
        assert!(outcome.needs_render);
        assert!(composer.has_command_picker());

        let outcome = composer.on_key_event(key(KeyCode::Enter));
        assert!(matches!(
            outcome.action,
            Some(PromptComposerAction::OpenConfig)
        ));
        assert_eq!(composer.buffer(), "");
        assert!(!composer.has_active_picker());
    }

    #[test]
    fn command_without_input_requests_submit_immediately() {
        let mut composer = PromptComposer::new();
        composer.set_available_commands(vec![CommandEntry {
            name: "status".into(),
            description: "status".into(),
            has_input: false,
            hint: None,
            builtin: false,
        }]);

        composer.on_key_event(key(KeyCode::Char('/')));
        let outcome = composer.on_key_event(key(KeyCode::Char('s')));
        assert!(outcome.needs_render);

        let outcome = composer.on_key_event(key(KeyCode::Enter));
        assert!(matches!(
            outcome.action,
            Some(PromptComposerAction::SubmitRequested { ref user_input, .. })
            if user_input == "/status"
        ));
        assert_eq!(composer.buffer(), "");
    }

    #[test]
    fn command_with_input_populates_prompt_without_submit() {
        let mut composer = PromptComposer::new();
        composer.set_available_commands(vec![CommandEntry {
            name: "search".into(),
            description: "Search code".into(),
            has_input: true,
            hint: Some("query".into()),
            builtin: false,
        }]);

        composer.on_key_event(key(KeyCode::Char('/')));
        composer.on_key_event(key(KeyCode::Char('s')));
        let outcome = composer.on_key_event(key(KeyCode::Enter));

        assert!(outcome.action.is_none());
        assert!(outcome.needs_render);
        assert_eq!(composer.buffer(), "/search ");
        assert!(!composer.has_active_picker());
    }

    #[test]
    fn submit_filters_unmentioned_attachments() {
        let mut composer = PromptComposer::new();
        composer.set_input("inspect @keep.rs now".to_string());
        composer.apply_file_selection(PathBuf::from("/tmp/keep.rs"), "keep.rs".to_string());
        composer.set_input("inspect @keep.rs now @skip.rs".to_string());
        composer.apply_file_selection(PathBuf::from("/tmp/skip.rs"), "skip.rs".to_string());
        composer.set_input("inspect @keep.rs now".to_string());

        let outcome = composer.prepare_submit();
        let Some(PromptComposerAction::SubmitRequested { attachments, .. }) = outcome.action else {
            panic!("expected submit request");
        };

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].display_name, "keep.rs");
        assert_eq!(attachments[0].path, PathBuf::from("/tmp/keep.rs"));
    }

    #[test]
    fn on_paste_closes_picker_and_inserts_text() {
        let mut composer = PromptComposer::new();
        composer.on_key_event(key(KeyCode::Char('@')));
        assert!(composer.has_file_picker());

        assert!(composer.on_paste("pasted text"));
        assert!(!composer.has_active_picker());
        assert_eq!(composer.buffer(), "@pasted text");
    }

    #[test]
    fn file_picker_cursor_tracks_query_length() {
        let mut composer = PromptComposer::new();
        composer.on_key_event(key(KeyCode::Char('@')));
        composer.on_key_event(key(KeyCode::Char('f')));
        composer.on_key_event(key(KeyCode::Char('o')));

        assert_eq!(composer.cursor_index(), 3);
    }

    #[test]
    fn command_picker_cursor_stays_in_prompt_row() {
        let mut composer = PromptComposer::new();
        composer.open_command_picker_with_entries(vec![CommandEntry {
            name: "config".into(),
            description: "Open config".into(),
            has_input: false,
            hint: None,
            builtin: true,
        }]);

        let context = RenderContext::new((120, 40));
        let output = composer.render(&context);
        let cursor = composer.cursor(&context);
        let input_row = output
            .iter()
            .position(|line| line.plain_text().contains("> "))
            .expect("input prompt should exist");

        assert_eq!(cursor.logical_row, input_row);
    }
}

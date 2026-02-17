use crate::components::command_picker::{CommandEntry, CommandPicker, CommandPickerAction};
use crate::components::config_menu::{ConfigChange, ConfigMenu, ConfigMenuAction};
use crate::components::config_picker::{ConfigPicker, ConfigPickerAction};
use crate::components::container::Container;
use crate::components::conversation_window::{ConversationWindow, StreamSegment};
use crate::components::file_picker::{FileMatch, FilePicker, FilePickerAction};
use crate::components::grid_loader::GridLoader;
use crate::components::input_prompt::InputPrompt;
use crate::components::status_line::StatusLine;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::tui::{
    Cursor, CursorComponent, HandlesInput, InputOutcome, RenderContext, RenderOutput,
};
use agent_client_protocol::SessionConfigOption;
use crossterm::event::KeyEvent;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

pub(crate) struct ScreenViewRenderProps<'a> {
    pub(crate) loader: &'a GridLoader,
    pub(crate) segments: &'a [StreamSegment],
    pub(crate) tool_call_statuses: &'a ToolCallStatuses,
    pub(crate) input: &'a str,
    pub(crate) active_mention_start: Option<usize>,
    pub(crate) agent_name: &'a str,
    pub(crate) model_display: Option<&'a str>,
    pub(crate) context_pct_left: Option<u8>,
}

pub enum ScreenViewAction {
    /// File mention confirmed — Renderer updates input_buffer + selected_mentions
    FileSelected { path: PathBuf, display_name: String },
    /// Command chosen — Renderer decides what to do (open config, format input, execute)
    CommandChosen(CommandEntry),
    /// Config change applied — Renderer calls prompt_handle.set_config_option()
    ConfigChanged(ConfigChange),
}

pub struct ScreenView {
    file_picker: Option<FilePicker>,
    command_picker: Option<CommandPicker>,
    config_menu: Option<ConfigMenu>,
    config_picker: Option<ConfigPicker>,
}

pub(crate) struct ScreenViewRoot<'view, 'props> {
    view: &'view ScreenView,
    props: ScreenViewRenderProps<'props>,
}

impl ScreenView {
    pub fn new() -> Self {
        Self {
            file_picker: None,
            command_picker: None,
            config_menu: None,
            config_picker: None,
        }
    }

    // ── Query methods ────────────────────────────────────────────────

    pub fn has_file_picker(&self) -> bool {
        self.file_picker.is_some()
    }

    pub fn has_command_picker(&self) -> bool {
        self.command_picker.is_some()
    }

    pub fn has_config_menu(&self) -> bool {
        self.config_menu.is_some()
    }

    pub fn has_config_picker(&self) -> bool {
        self.config_picker.is_some()
    }

    pub fn config_menu_selected_index(&self) -> Option<usize> {
        self.config_menu.as_ref().map(|m| m.selected_index)
    }

    pub fn config_picker_config_id(&self) -> Option<&str> {
        self.config_picker.as_ref().map(|p| p.config_id.as_str())
    }

    pub fn file_picker_selected_display_name(&self) -> Option<String> {
        self.file_picker
            .as_ref()
            .and_then(|p| p.combobox.selected().map(|f| f.display_name.clone()))
    }

    pub fn command_picker_match_names(&self) -> Vec<&str> {
        self.command_picker
            .as_ref()
            .map(|p| p.combobox.matches.iter().map(|m| m.name.as_str()).collect())
            .unwrap_or_default()
    }

    // ── Mutation methods ─────────────────────────────────────────────

    pub fn open_file_picker(&mut self) {
        self.file_picker = Some(FilePicker::new());
    }

    pub fn open_file_picker_with_matches(&mut self, matches: Vec<FileMatch>) {
        self.file_picker = Some(FilePicker::from_matches(matches));
    }

    pub fn open_command_picker(&mut self, commands: Vec<CommandEntry>) {
        self.command_picker = Some(CommandPicker::new(commands));
    }

    pub fn open_config_menu(&mut self, options: &[SessionConfigOption]) {
        self.config_menu = Some(ConfigMenu::from_config_options(options));
    }

    pub fn open_config_picker_for(&mut self, config_id: &str) -> bool {
        let Some(menu) = self.config_menu.as_ref() else {
            return false;
        };
        let Some(entry) = menu.entry_by_id(config_id) else {
            return false;
        };
        let Some(picker) = ConfigPicker::from_entry(entry) else {
            return false;
        };
        self.config_picker = Some(picker);
        true
    }

    /// Close file, command, and config pickers (for paste — leaves config_menu open)
    pub fn close_all_pickers(&mut self) {
        self.file_picker = None;
        self.command_picker = None;
        self.config_picker = None;
    }

    /// Close file + command pickers (for execute_input)
    pub fn close_input_pickers(&mut self) {
        self.file_picker = None;
        self.command_picker = None;
    }

    pub fn update_config_menu(&mut self, options: &[SessionConfigOption]) {
        if let Some(ref mut menu) = self.config_menu {
            menu.update_options(options);
        }
    }

    // ── Key dispatch ─────────────────────────────────────────────────

    pub fn handle_key(
        &mut self,
        key_event: KeyEvent,
        input_buffer: &mut String,
    ) -> InputOutcome<ScreenViewAction> {
        // 1. file_picker — can fall through if not consumed
        if let Some(ref mut picker) = self.file_picker {
            let outcome = picker.handle_key(key_event, input_buffer);
            if outcome.consumed {
                return self.handle_file_picker_outcome(outcome);
            }
        }
        // 2. command_picker — exclusive
        if let Some(ref mut picker) = self.command_picker {
            let outcome = picker.handle_key(key_event, input_buffer);
            return self.handle_command_picker_outcome(outcome);
        }
        // 3. config_picker — exclusive
        if let Some(ref mut picker) = self.config_picker {
            let outcome = picker.handle_key(key_event, input_buffer);
            return self.handle_config_picker_outcome(outcome);
        }
        // 4. config_menu — exclusive
        if let Some(ref mut menu) = self.config_menu {
            let outcome = menu.handle_key(key_event, input_buffer);
            return self.handle_config_menu_outcome(outcome);
        }
        InputOutcome::ignored()
    }

    // ── Root ─────────────────────────────────────────────────────────

    pub(crate) fn root<'view, 'props>(
        &'view self,
        props: ScreenViewRenderProps<'props>,
    ) -> ScreenViewRoot<'view, 'props> {
        ScreenViewRoot { view: self, props }
    }

    fn input_cursor_index(&self, input: &str, active_mention_start: Option<usize>) -> usize {
        if let Some(ref picker) = self.file_picker {
            let at_pos = active_mention_start.unwrap_or(input.len());
            at_pos + 1 + picker.combobox.query.len()
        } else {
            input.len()
        }
    }

    fn config_picker_cursor_col(picker: &ConfigPicker) -> usize {
        let prefix = format!("  {} search: ", picker.title);
        UnicodeWidthStr::width(prefix.as_str())
            + UnicodeWidthStr::width(picker.combobox.query.as_str())
    }

    fn command_picker_cursor_col(picker: &CommandPicker) -> usize {
        let prefix = "  / search: ";
        UnicodeWidthStr::width(prefix) + UnicodeWidthStr::width(picker.combobox.query.as_str())
    }

    // ── Internal outcome handlers ────────────────────────────────────

    fn handle_file_picker_outcome(
        &mut self,
        outcome: InputOutcome<FilePickerAction>,
    ) -> InputOutcome<ScreenViewAction> {
        let action = match outcome.action {
            Some(FilePickerAction::Close) => {
                self.file_picker = None;
                None
            }
            Some(FilePickerAction::ConfirmSelection) => {
                let picker = self.file_picker.take();
                picker
                    .and_then(|p| p.combobox.selected().cloned())
                    .map(|selected| ScreenViewAction::FileSelected {
                        path: selected.path,
                        display_name: selected.display_name,
                    })
            }
            None => None,
        };
        InputOutcome {
            consumed: true,
            needs_render: outcome.needs_render,
            action,
        }
    }

    fn handle_command_picker_outcome(
        &mut self,
        outcome: InputOutcome<CommandPickerAction>,
    ) -> InputOutcome<ScreenViewAction> {
        let action = match outcome.action {
            Some(CommandPickerAction::CloseAndClearInput) => {
                self.command_picker = None;
                None
            }
            Some(CommandPickerAction::CommandChosen(cmd)) => {
                self.command_picker = None;
                Some(ScreenViewAction::CommandChosen(cmd))
            }
            None => None,
        };
        InputOutcome {
            consumed: outcome.consumed,
            needs_render: outcome.needs_render,
            action,
        }
    }

    fn handle_config_picker_outcome(
        &mut self,
        outcome: InputOutcome<ConfigPickerAction>,
    ) -> InputOutcome<ScreenViewAction> {
        let action = match outcome.action {
            Some(ConfigPickerAction::Close) => {
                self.config_picker = None;
                None
            }
            Some(ConfigPickerAction::ApplySelection(confirmed_change)) => {
                self.config_picker = None;
                confirmed_change.map(ScreenViewAction::ConfigChanged)
            }
            None => None,
        };
        InputOutcome {
            consumed: outcome.consumed,
            needs_render: outcome.needs_render,
            action,
        }
    }

    fn handle_config_menu_outcome(
        &mut self,
        outcome: InputOutcome<ConfigMenuAction>,
    ) -> InputOutcome<ScreenViewAction> {
        if let Some(action) = outcome.action {
            match action {
                ConfigMenuAction::CloseAll => {
                    self.config_menu = None;
                    self.config_picker = None;
                }
                ConfigMenuAction::OpenSelectedPicker => {
                    self.config_picker = self
                        .config_menu
                        .as_ref()
                        .and_then(|menu| menu.selected_entry())
                        .and_then(ConfigPicker::from_entry);
                }
            }
        }
        InputOutcome {
            consumed: outcome.consumed,
            needs_render: outcome.needs_render,
            action: None,
        }
    }
}

impl CursorComponent for ScreenViewRoot<'_, '_> {
    fn render_with_cursor(&self, context: &RenderContext) -> RenderOutput {
        let conversation_window = ConversationWindow {
            loader: self.props.loader,
            segments: self.props.segments,
            tool_call_statuses: self.props.tool_call_statuses,
        };
        let input_prompt = InputPrompt {
            input: self.props.input,
            cursor_index: self
                .view
                .input_cursor_index(self.props.input, self.props.active_mention_start),
        };
        let input_layout = input_prompt.layout(context);
        let status_line = StatusLine {
            agent_name: self.props.agent_name,
            model_display: self.props.model_display,
            context_pct_left: self.props.context_pct_left,
        };

        let mut container: Container<'_> =
            Container::new(vec![&conversation_window, &input_prompt]);
        let input_component_index = 1;

        if let Some(ref picker) = self.view.file_picker {
            container.push(picker);
        }

        let command_picker_index = if let Some(ref picker) = self.view.command_picker {
            let idx = container.len();
            container.push(picker);
            Some(idx)
        } else {
            None
        };
        let command_picker_col = self
            .view
            .command_picker
            .as_ref()
            .map(ScreenView::command_picker_cursor_col);

        let config_picker_index = if let Some(ref picker) = self.view.config_picker {
            let idx = container.len();
            container.push(picker);
            Some(idx)
        } else {
            if let Some(ref menu) = self.view.config_menu {
                container.push(menu);
            }
            None
        };
        let config_picker_col = self
            .view
            .config_picker
            .as_ref()
            .map(ScreenView::config_picker_cursor_col);

        container.push(&status_line);
        let (lines, offsets) = container.render_with_offsets(context);

        let mut cursor = Cursor {
            logical_row: offsets[input_component_index] + input_layout.cursor_row,
            col: input_layout.cursor_col as usize,
        };

        if let Some(idx) = command_picker_index {
            cursor = Cursor {
                logical_row: offsets[idx],
                col: command_picker_col.unwrap_or(0),
            };
        }

        if let Some(idx) = config_picker_index {
            cursor = Cursor {
                logical_row: offsets[idx],
                col: config_picker_col.unwrap_or(0),
            };
        }

        RenderOutput { lines, cursor }
    }
}

impl Default for ScreenView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::tool_call_statuses::ToolCallStatuses;

    fn props<'a>(
        loader: &'a GridLoader,
        statuses: &'a ToolCallStatuses,
        input: &'a str,
    ) -> ScreenViewRenderProps<'a> {
        static EMPTY_SEGMENTS: [StreamSegment; 0] = [];
        ScreenViewRenderProps {
            loader,
            segments: &EMPTY_SEGMENTS,
            tool_call_statuses: statuses,
            input,
            active_mention_start: None,
            agent_name: "test-agent",
            model_display: None,
            context_pct_left: None,
        }
    }

    #[test]
    fn command_picker_cursor_targets_picker_header() {
        let mut view = ScreenView::new();
        view.open_command_picker(vec![CommandEntry {
            name: "config".to_string(),
            description: "Open config".to_string(),
            has_input: false,
            hint: None,
            builtin: true,
        }]);
        let context = RenderContext::new((120, 40));
        let loader = GridLoader::default();
        let statuses = ToolCallStatuses::new();

        let output = view
            .root(props(&loader, &statuses, ""))
            .render_with_cursor(&context);
        let row = output
            .lines
            .iter()
            .position(|line| line.as_str().contains("  / search: "))
            .expect("command picker header should exist");
        assert_eq!(output.cursor.logical_row, row);
    }

    #[test]
    fn config_picker_takes_precedence_over_config_menu() {
        let mut view = ScreenView::new();
        let opts = vec![agent_client_protocol::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![agent_client_protocol::SessionConfigSelectOption::new(
                "m1", "M1",
            )],
        )];
        view.open_config_menu(&opts);
        view.open_config_picker_for("model");
        let context = RenderContext::new((120, 40));
        let loader = GridLoader::default();
        let statuses = ToolCallStatuses::new();

        let output = view
            .root(props(&loader, &statuses, ""))
            .render_with_cursor(&context);
        let has_menu_row = output
            .lines
            .iter()
            .any(|line| line.as_str().contains("Model: M1"));
        assert!(
            !has_menu_row,
            "config menu should be hidden when picker is open"
        );
    }
}

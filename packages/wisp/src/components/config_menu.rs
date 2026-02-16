use crate::tui::{Component, Line, RenderContext};
use agent_client_protocol::{
    SessionConfigKind, SessionConfigOption, SessionConfigSelectOptions,
};
use crossterm::style::Stylize;

pub struct ConfigMenu {
    pub options: Vec<ConfigMenuEntry>,
    pub selected_index: usize,
}

pub struct ConfigMenuEntry {
    pub config_id: String,
    pub title: String,
    pub values: Vec<ConfigMenuValue>,
    pub current_value_index: usize,
}

pub struct ConfigMenuValue {
    pub value: String,
    pub name: String,
}

pub struct ConfigChange {
    pub config_id: String,
    pub new_value: String,
}

pub struct ConfigMenuComponent<'a> {
    pub menu: &'a ConfigMenu,
}

impl Component for ConfigMenuComponent<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        if self.menu.options.is_empty() {
            return vec![Line::new("  (no config options)".to_string())];
        }

        self.menu
            .options
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let selected = i == self.menu.selected_index;
                let prefix = if selected { "▶ " } else { "  " };
                let current_name = entry
                    .values
                    .get(entry.current_value_index)
                    .map(|v| v.name.as_str())
                    .unwrap_or("?");
                let text = format!("{}{}: < {} >", prefix, entry.title, current_name);
                if selected {
                    Line::new(text.with(context.theme.primary).to_string())
                } else {
                    Line::new(text)
                }
            })
            .collect()
    }
}

impl ConfigMenu {
    pub fn from_config_options(options: &[SessionConfigOption]) -> Self {
        let entries: Vec<ConfigMenuEntry> = options
            .iter()
            .filter_map(|opt| {
                let SessionConfigKind::Select(ref select) = opt.kind else {
                    return None;
                };
                let flat_options = match &select.options {
                    SessionConfigSelectOptions::Ungrouped(opts) => opts.clone(),
                    SessionConfigSelectOptions::Grouped(groups) => {
                        groups.iter().flat_map(|g| g.options.clone()).collect()
                    }
                    _ => return None,
                };
                if flat_options.is_empty() {
                    return None;
                }
                let current_value_index = flat_options
                    .iter()
                    .position(|o| o.value == select.current_value)
                    .unwrap_or(0);
                let values = flat_options
                    .into_iter()
                    .map(|o| ConfigMenuValue {
                        value: o.value.0.to_string(),
                        name: o.name,
                    })
                    .collect();
                Some(ConfigMenuEntry {
                    config_id: opt.id.0.to_string(),
                    title: opt.name.clone(),
                    values,
                    current_value_index,
                })
            })
            .collect();

        Self {
            options: entries,
            selected_index: 0,
        }
    }

    pub fn move_selection_up(&mut self) {
        if !self.options.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.options.len() - 1;
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        if !self.options.is_empty() {
            if self.selected_index < self.options.len() - 1 {
                self.selected_index += 1;
            } else {
                self.selected_index = 0;
            }
        }
    }

    pub fn cycle_value_right(&mut self) -> Option<ConfigChange> {
        let entry = self.options.get_mut(self.selected_index)?;
        if entry.values.len() <= 1 {
            return None;
        }
        let new_index = if entry.current_value_index < entry.values.len() - 1 {
            entry.current_value_index + 1
        } else {
            0
        };
        entry.current_value_index = new_index;
        Some(ConfigChange {
            config_id: entry.config_id.clone(),
            new_value: entry.values[new_index].value.clone(),
        })
    }

    pub fn cycle_value_left(&mut self) -> Option<ConfigChange> {
        let entry = self.options.get_mut(self.selected_index)?;
        if entry.values.len() <= 1 {
            return None;
        }
        let new_index = if entry.current_value_index > 0 {
            entry.current_value_index - 1
        } else {
            entry.values.len() - 1
        };
        entry.current_value_index = new_index;
        Some(ConfigChange {
            config_id: entry.config_id.clone(),
            new_value: entry.values[new_index].value.clone(),
        })
    }

    pub fn update_options(&mut self, options: &[SessionConfigOption]) {
        *self = Self::from_config_options(options);
        if self.selected_index >= self.options.len() {
            self.selected_index = self.options.len().saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{
        SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    };

    fn make_select_option(
        id: &str,
        name: &str,
        current: &str,
        values: &[(&str, &str)],
    ) -> SessionConfigOption {
        let options: Vec<SessionConfigSelectOption> = values
            .iter()
            .map(|(v, n)| SessionConfigSelectOption::new(v.to_string(), n.to_string()))
            .collect();
        SessionConfigOption::select(id.to_string(), name.to_string(), current.to_string(), options)
    }

    #[test]
    fn from_config_options_builds_entries() {
        let opts = vec![
            make_select_option("model", "Model", "gpt-4o", &[("gpt-4o", "GPT-4o"), ("claude", "Claude")]),
            make_select_option("mode", "Mode", "code", &[("code", "Code"), ("chat", "Chat")]),
        ];
        let menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.options.len(), 2);
        assert_eq!(menu.options[0].config_id, "model");
        assert_eq!(menu.options[0].current_value_index, 0);
        assert_eq!(menu.options[1].config_id, "mode");
    }

    #[test]
    fn from_config_options_finds_current_value() {
        let opts = vec![make_select_option(
            "model", "Model", "claude",
            &[("gpt-4o", "GPT-4o"), ("claude", "Claude"), ("llama", "Llama")],
        )];
        let menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.options[0].current_value_index, 1);
    }

    #[test]
    fn navigation_wraps_around() {
        let opts = vec![
            make_select_option("a", "A", "v1", &[("v1", "V1")]),
            make_select_option("b", "B", "v1", &[("v1", "V1")]),
            make_select_option("c", "C", "v1", &[("v1", "V1")]),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.selected_index, 0);

        menu.move_selection_up();
        assert_eq!(menu.selected_index, 2);

        menu.move_selection_down();
        assert_eq!(menu.selected_index, 0);

        menu.move_selection_down();
        menu.move_selection_down();
        menu.move_selection_down();
        assert_eq!(menu.selected_index, 0);
    }

    #[test]
    fn cycle_value_right_wraps() {
        let opts = vec![make_select_option(
            "model", "Model", "a",
            &[("a", "A"), ("b", "B"), ("c", "C")],
        )];
        let mut menu = ConfigMenu::from_config_options(&opts);

        let change = menu.cycle_value_right().unwrap();
        assert_eq!(change.new_value, "b");
        assert_eq!(menu.options[0].current_value_index, 1);

        menu.cycle_value_right();
        assert_eq!(menu.options[0].current_value_index, 2);

        let change = menu.cycle_value_right().unwrap();
        assert_eq!(change.new_value, "a");
        assert_eq!(menu.options[0].current_value_index, 0);
    }

    #[test]
    fn cycle_value_left_wraps() {
        let opts = vec![make_select_option(
            "model", "Model", "a",
            &[("a", "A"), ("b", "B"), ("c", "C")],
        )];
        let mut menu = ConfigMenu::from_config_options(&opts);

        let change = menu.cycle_value_left().unwrap();
        assert_eq!(change.new_value, "c");
        assert_eq!(menu.options[0].current_value_index, 2);
    }

    #[test]
    fn single_value_returns_none() {
        let opts = vec![make_select_option("x", "X", "only", &[("only", "Only")])];
        let mut menu = ConfigMenu::from_config_options(&opts);
        assert!(menu.cycle_value_right().is_none());
        assert!(menu.cycle_value_left().is_none());
    }

    #[test]
    fn update_options_clamps_index() {
        let opts = vec![
            make_select_option("a", "A", "v1", &[("v1", "V1")]),
            make_select_option("b", "B", "v1", &[("v1", "V1")]),
            make_select_option("c", "C", "v1", &[("v1", "V1")]),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);
        menu.selected_index = 2;

        let fewer = vec![make_select_option("a", "A", "v1", &[("v1", "V1")])];
        menu.update_options(&fewer);
        assert_eq!(menu.selected_index, 0);
    }

    #[test]
    fn component_renders_selected_row() {
        let opts = vec![
            make_select_option("model", "Model", "gpt-4o", &[("gpt-4o", "GPT-4o"), ("claude", "Claude")]),
            make_select_option("mode", "Mode", "code", &[("code", "Code"), ("chat", "Chat")]),
        ];
        let menu = ConfigMenu::from_config_options(&opts);
        let component = ConfigMenuComponent { menu: &menu };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);

        assert_eq!(lines.len(), 2);
        // First line is selected (contains ▶)
        assert!(lines[0].as_str().contains("▶"));
        assert!(lines[0].as_str().contains("Model"));
        assert!(lines[0].as_str().contains("GPT-4o"));
        // Second line is not selected
        assert!(lines[1].as_str().contains("Mode"));
        assert!(lines[1].as_str().contains("Code"));
        assert!(!lines[1].as_str().contains("▶"));
    }

    #[test]
    fn empty_options_renders_placeholder() {
        let menu = ConfigMenu::from_config_options(&[]);
        let component = ConfigMenuComponent { menu: &menu };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("no config options"));
    }

    #[test]
    fn from_config_options_skips_empty_values() {
        let empty = SessionConfigOption::select("x", "X", "v", Vec::<SessionConfigSelectOption>::new());
        let opts = vec![
            empty,
            make_select_option("model", "Model", "a", &[("a", "A")]),
        ];
        let menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.options.len(), 1);
        assert_eq!(menu.options[0].config_id, "model");
    }

    #[test]
    fn from_config_options_with_category() {
        let opt = make_select_option(
            "model", "Model", "gpt-4o",
            &[("gpt-4o", "GPT-4o")],
        ).category(SessionConfigOptionCategory::Model);
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert_eq!(menu.options.len(), 1);
        assert_eq!(menu.options[0].title, "Model");
    }
}

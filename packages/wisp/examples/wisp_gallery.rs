use std::time::Duration;

use agent_client_protocol::{PlanEntry, PlanEntryPriority, PlanEntryStatus};
use tui::{
    Component, CrosstermEvent, Event, Frame, Gallery, GalleryMessage, Line, MouseCapture, Renderer, TerminalSession,
    Theme, ViewContext, spawn_terminal_event_task, terminal_size,
};
use wisp::components::command_picker::{CommandEntry, CommandPicker};
use wisp::components::file_picker::{FileMatch, FilePicker};
use wisp::components::plan_view::PlanView;
use wisp::components::progress_indicator::ProgressIndicator;
use wisp::components::status_line::StatusLine;
use wisp::components::text_input::TextInput;
use wisp::components::thought_message::ThoughtMessage;
use wisp::components::tool_call_status_view::{ToolCallStatus, ToolCallStatusView};
use wisp::keybindings::Keybindings;

enum WispStory {
    TextInput(TextInput),
    CommandPicker(CommandPicker),
    FilePicker(FilePicker),
    ProgressIndicator(ProgressIndicatorStory),
    ToolCallStatus(ToolCallStatusStory),
    ThoughtMessage(ThoughtMessageStory),
    StatusLine(StatusLineStory),
    PlanView(PlanViewStory),
}

impl Component for WispStory {
    type Message = ();

    async fn on_event(&mut self, event: &Event) -> Option<Vec<()>> {
        match self {
            Self::TextInput(c) => c.on_event(event).await.map(|_| vec![]),
            Self::CommandPicker(c) => c.on_event(event).await.map(|_| vec![]),
            Self::FilePicker(c) => c.on_event(event).await.map(|_| vec![]),
            Self::ProgressIndicator(c) => c.on_event(event).await,
            Self::ToolCallStatus(c) => c.on_event(event).await,
            Self::ThoughtMessage(c) => c.on_event(event).await,
            Self::StatusLine(c) => c.on_event(event).await,
            Self::PlanView(c) => c.on_event(event).await,
        }
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        match self {
            Self::TextInput(c) => c.render(ctx),
            Self::CommandPicker(c) => c.render(ctx),
            Self::FilePicker(c) => c.render(ctx),
            Self::ProgressIndicator(c) => c.render(ctx),
            Self::ToolCallStatus(c) => c.render(ctx),
            Self::ThoughtMessage(c) => c.render(ctx),
            Self::StatusLine(c) => c.render(ctx),
            Self::PlanView(c) => c.render(ctx),
        }
    }
}

struct ProgressIndicatorStory {
    indicator: ProgressIndicator,
}

impl Component for ProgressIndicatorStory {
    type Message = ();

    async fn on_event(&mut self, event: &Event) -> Option<Vec<()>> {
        if let Event::Tick = event {
            self.indicator.on_tick();
        }
        Some(vec![])
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        Frame::new(self.indicator.render(ctx))
    }
}

struct ToolCallStatusStory {
    tick: u16,
}

impl Component for ToolCallStatusStory {
    type Message = ();

    async fn on_event(&mut self, event: &Event) -> Option<Vec<()>> {
        if let Event::Tick = event {
            self.tick = self.tick.wrapping_add(1);
        }
        Some(vec![])
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let running = ToolCallStatus::Running;
        let success = ToolCallStatus::Success;
        let error = ToolCallStatus::Error("exit code 1".into());

        let mut lines = Vec::new();

        lines.extend(
            ToolCallStatusView {
                name: "Read",
                arguments: "src/main.rs",
                display_value: None,
                diff_preview: None,
                status: &running,
                tick: self.tick,
            }
            .render(ctx),
        );
        lines.push(Line::default());

        lines.extend(
            ToolCallStatusView {
                name: "Write",
                arguments: "src/lib.rs",
                display_value: Some("src/lib.rs"),
                diff_preview: None,
                status: &success,
                tick: self.tick,
            }
            .render(ctx),
        );
        lines.push(Line::default());

        lines.extend(
            ToolCallStatusView {
                name: "Bash",
                arguments: "cargo test",
                display_value: None,
                diff_preview: None,
                status: &error,
                tick: self.tick,
            }
            .render(ctx),
        );

        Frame::new(lines)
    }
}

struct ThoughtMessageStory {
    text: String,
}

impl Component for ThoughtMessageStory {
    type Message = ();

    async fn on_event(&mut self, _event: &Event) -> Option<Vec<()>> {
        Some(vec![])
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let view = ThoughtMessage { text: &self.text };
        Frame::new(view.render(ctx))
    }
}

struct StatusLineStory;

impl Component for StatusLineStory {
    type Message = ();

    async fn on_event(&mut self, _event: &Event) -> Option<Vec<()>> {
        Some(vec![])
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let status = StatusLine {
            agent_name: "aether",
            config_options: &[],
            context_pct_left: Some(72),
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        Frame::new(status.render(ctx))
    }
}

struct PlanViewStory {
    entries: Vec<PlanEntry>,
}

impl Component for PlanViewStory {
    type Message = ();

    async fn on_event(&mut self, _event: &Event) -> Option<Vec<()>> {
        Some(vec![])
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let view = PlanView { entries: &self.entries };
        Frame::new(view.render(ctx))
    }
}

fn sample_commands() -> Vec<CommandEntry> {
    vec![
        CommandEntry {
            name: "clear".into(),
            description: "Clear screen and start a new session".into(),
            has_input: false,
            hint: None,
            builtin: true,
        },
        CommandEntry {
            name: "settings".into(),
            description: "Open settings".into(),
            has_input: false,
            hint: None,
            builtin: true,
        },
        CommandEntry {
            name: "search".into(),
            description: "Search code in the project".into(),
            has_input: true,
            hint: Some("query".into()),
            builtin: false,
        },
        CommandEntry {
            name: "web".into(),
            description: "Browse the web".into(),
            has_input: true,
            hint: Some("url".into()),
            builtin: false,
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

fn sample_plan_entries() -> Vec<PlanEntry> {
    vec![
        PlanEntry::new("Research AI agent patterns", PlanEntryPriority::Medium, PlanEntryStatus::Completed),
        PlanEntry::new("Implement task tracking", PlanEntryPriority::High, PlanEntryStatus::Completed),
        PlanEntry::new("Build component gallery", PlanEntryPriority::Medium, PlanEntryStatus::InProgress),
        PlanEntry::new("Write integration tests", PlanEntryPriority::Medium, PlanEntryStatus::Pending),
        PlanEntry::new("Update documentation", PlanEntryPriority::Low, PlanEntryStatus::Pending),
    ]
}

fn sample_files() -> Vec<FileMatch> {
    vec![
        FileMatch { path: "src/main.rs".into(), display_name: "src/main.rs".into() },
        FileMatch { path: "src/lib.rs".into(), display_name: "src/lib.rs".into() },
        FileMatch { path: "src/components/mod.rs".into(), display_name: "src/components/mod.rs".into() },
        FileMatch { path: "src/components/gallery.rs".into(), display_name: "src/components/gallery.rs".into() },
        FileMatch { path: "src/rendering/renderer.rs".into(), display_name: "src/rendering/renderer.rs".into() },
        FileMatch { path: "Cargo.toml".into(), display_name: "Cargo.toml".into() },
        FileMatch { path: "README.md".into(), display_name: "README.md".into() },
    ]
}

fn stories() -> Vec<(String, WispStory)> {
    vec![
        (
            "TextInput".into(),
            WispStory::TextInput(TextInput::new(Keybindings::default())),
        ),
        (
            "CommandPicker".into(),
            WispStory::CommandPicker(CommandPicker::new(sample_commands())),
        ),
        (
            "FilePicker".into(),
            WispStory::FilePicker(FilePicker::new_with_entries(sample_files())),
        ),
        (
            "ProgressIndicator".into(),
            WispStory::ProgressIndicator({
                let mut indicator = ProgressIndicator::default();
                indicator.update(1, 3, true);
                ProgressIndicatorStory { indicator }
            }),
        ),
        (
            "ToolCallStatus".into(),
            WispStory::ToolCallStatus(ToolCallStatusStory { tick: 0 }),
        ),
        (
            "ThoughtMessage".into(),
            WispStory::ThoughtMessage(ThoughtMessageStory {
                text: "Let me analyze the codebase structure.\nI see several modules that need refactoring.\nStarting with the authentication layer."
                    .into(),
            }),
        ),
        ("StatusLine".into(), WispStory::StatusLine(StatusLineStory)),
        (
            "PlanView".into(),
            WispStory::PlanView(PlanViewStory {
                entries: sample_plan_entries(),
            }),
        ),
    ]
}

#[tokio::main]
async fn main() {
    let mut gallery = Gallery::new(stories());
    let size = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(std::io::stdout(), Theme::default(), size);
    let _session = TerminalSession::new(true, MouseCapture::Disabled).unwrap();
    let mut events = spawn_terminal_event_task();
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    renderer.render_frame(|ctx| gallery.render(ctx)).unwrap();

    loop {
        tokio::select! {
            Some(raw) = events.recv() => {
                if let CrosstermEvent::Resize(cols, rows) = &raw {
                    renderer.on_resize((*cols, *rows));
                }
                if let Ok(event) = Event::try_from(raw) {
                    if let Some(msgs) = gallery.on_event(&event).await
                        && msgs.iter().any(|m| matches!(m, GalleryMessage::Quit))
                    {
                        return;
                    }
                    renderer.render_frame(|ctx| gallery.render(ctx)).unwrap();
                }
            }
            _ = tick.tick() => {
                gallery.on_event(&Event::Tick).await;
                renderer.render_frame(|ctx| gallery.render(ctx)).unwrap();
            }
        }
    }
}

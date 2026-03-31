The main chat UI screen.

`ConversationScreen` composes the core conversation experience from several sub-components:

- `ConversationBuffer` / `ConversationWindow` — scrollable message history
- `PromptComposer` — text input with file mentions
- `ToolCallStatuses` — live progress for running tool calls
- `PlanTracker` — visual plan step progress
- `ProgressIndicator` — animated spinner while waiting

# Modals

The screen can display one modal at a time:

- **`ElicitationForm`** — agent-requested structured input (e.g. confirmation dialogs)
- **`SessionPicker`** — list of previous sessions for `/resume`

# See also

- [`App`](crate::components::app::App) — the parent that owns this screen
- [`ConversationScreenMessage`] — messages produced by event handling

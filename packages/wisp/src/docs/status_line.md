The bottom status bar showing agent state at a glance.

Renders a single line with left-aligned identity info and right-aligned indicators:

**Left side:**
- Agent name
- Current mode/profile (if configured)
- Active model name

**Right side:**
- Reasoning effort bar (visual level indicator)
- Context window usage bar
- Unhealthy MCP server count (when not waiting for a response)

# See also

- [`App`](crate::components::app::App) — constructs this view each render cycle
- [`Keybindings`](crate::keybindings::Keybindings) — `Tab` cycles reasoning, `Shift+Tab` cycles mode

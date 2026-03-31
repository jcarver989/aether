Wisp settings management and theme loading.

Wisp has two tiers of configuration:

1. **Wisp settings** — persisted in `~/.wisp/settings.json` (or `$WISP_HOME/settings.json`). Currently controls theme selection via [`WispSettings`].
2. **Agent settings** — model, reasoning effort, MCP servers, etc. These are advertised by the agent over ACP and edited through the in-app [`SettingsOverlay`](overlay::SettingsOverlay).

# Theme resolution

1. [`load_or_create_settings`] reads (or creates) the settings file.
2. [`load_theme`] checks [`ThemeSettings::file`](WispSettings) for a `.tmTheme` filename.
3. [`resolve_theme_file_path`] validates the filename (must be a bare basename, no path traversal) and resolves it to `~/.wisp/themes/<name>`.
4. [`tui::Theme::load_from_path`](tui::Theme::load_from_path) parses the file, falling back to the default theme on error.

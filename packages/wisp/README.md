# Wisp

Wisp is a Terminal User Interface (TUI) application designed to provide an ethereal AI assistant experience. It leverages the Aether framework to deliver powerful AI capabilities with a clean, colorful terminal interface.

## Features

- **Colorful TUI**: Beautiful terminal interface with custom color schemes
- **AI Assistant**: Powered by Aether's agent system and local LLM capabilities
- **Tool Integration**: Supports tool calling for extended functionality
- **Agent Management**: Custom agents defined via `.aether/settings.json`
- **Real-time Response**: Streaming responses with progress indicators

## Installation

To build and install Wisp, you'll need Rust installed on your system. Then run:

```bash
 cargo install --path .
```

## Usage

Wisp can be invoked with a prompt to interact with the AI assistant:

```bash
wisp "Implement a binary search tree in Rust"
```

Or for a more complex interaction:

```bash
wisp "Explain how async/await works in Rust and show an example"
```

## Agent Configuration

Wisp uses the centralized `.aether/settings.json` configuration. See the main [Aether README](../../README.md) for agent configuration details.

## Settings

Wisp stores its settings in `~/.wisp/settings.json` (override with `WISP_HOME` env var). The file is created with defaults on first run.

### Custom Themes

Load a TextMate/Shiki-compatible JSON theme to restyle the UI and syntax highlighting:

```json
{
  "theme": {
    "file": "my-theme.json"
  }
}
```

Theme files are resolved from `~/.wisp/themes/`. Only basenames are accepted — path traversal (e.g. `../escape.json`) is rejected.

A theme file uses the TextMate JSON format:

```json
{
  "name": "My Theme",
  "settings": {
    "foreground": "#BFBDB6",
    "background": "#10141C",
    "selection": "#3388FF",
    "caret": "#E6B450"
  },
  "scopes": [
    { "scope": "comment", "settings": { "foreground": "#ACB6BF", "fontStyle": "italic" } },
    { "scope": "string", "settings": { "foreground": "#AAD94C" } }
  ]
}
```

Invalid or missing theme files fall back to the built-in defaults.

### Keyboard Shortcuts

- **Tab** — Cycle reasoning effort level (none → low → medium → high).
- **Shift+Tab** — Cycle the first ACP config option that is a select option in the `SessionConfigOptionCategory::Mode` category (e.g. mode profiles from `aether-acp`).

## Requirements

- Rust 1.70 or later
- An ACP-compatible agent (e.g. `aether-acp`)

## License

This project is licensed under the MIT License - see the LICENSE file for details.

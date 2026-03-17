# Wisp

A TUI for AI coding agents via the Agent Client Protocol (ACP).

## Install

```bash
cargo install --path packages/wisp
```

## Run

```bash
wisp
```

## Keybindings

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Esc` | Cancel |
| `Ctrl+C` | Exit |
| `Tab` | Cycle reasoning effort |
| `Shift+Tab` | Cycle mode/profile |
| `/` | Command picker |
| `@` | File picker |
| `Ctrl+G` | Toggle git diff |

## Slash Commands

Type `/` in the input to open the command picker. Built-in commands:

| Command | Description |
|---------|-------------|
| `/clear` | Clear screen and start a new session |
| `/settings` | Open settings overlay |
| `/resume` | Resume a previous session |

Additional commands may be available from the agent (e.g., `/search`, `/web`).

## Logs

Debug logs are written to `/tmp/wisp-logs/wisp.log.YYYY-MM-DD` by default. Override with:

```bash
wisp --log-dir ~/logs
```

## Settings

Wisp has two kinds of settings:

1. **Wisp settings** (`~/.wisp/settings.json`) — UI preferences like themes
2. **Agent settings** — Model, reasoning effort, MCP servers, etc. These come from the agent and are configured in-app via `/settings`

Override the Wisp home directory with `WISP_HOME` environment variable.

### Themes

Place `.tmTheme` files in `~/.wisp/themes/`:

```bash
mkdir -p ~/.wisp/themes
curl -o ~/.wisp/themes/catppuccin.tmTheme https://example.com/catppuccin.tmTheme
```

Then set in `~/.wisp/settings.json`:

```json
{
  "theme": { "file": "catppuccin.tmTheme" }
}
```

Remove `"file"` or set it to `null` to use the default theme.

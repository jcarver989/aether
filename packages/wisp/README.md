# wisp

A terminal interface for AI coding agents, built on the Agent Client Protocol (ACP).

Wisp launches an ACP-compatible agent as a subprocess, streams the conversation in real time, and gives you built-in git diff viewing, file attachments, session management, and a settings overlay — all without leaving the terminal.

## Quick start

```bash
cargo install --path packages/wisp
wisp                       # launches the default agent ("aether acp")
wisp --agent "my-agent"    # launch a custom ACP agent
```

The `--agent` flag accepts any shell command that speaks ACP over stdio.

## How it works

```text
CLI args ──→ RuntimeState (ACP handshake + theme) ──→ App event loop ──→ Renderer
                                                           │
                            ┌──────────────────────────────┼──────────────────┐
                            ▼                              ▼                  ▼
                    terminal events               ACP events (stream)      tick
                    (keys, resize)                (text, tool calls,     (100 ms)
                            │                      plans, thoughts)         │
                            ▼                              ▼                ▼
                        on_event ─────────────────→ on_acp_event ────→ on_tick
                            │                              │                │
                            └──────────────┬───────────────┘                │
                                           ▼                                │
                                    Renderer::render_frame  ◀───────────────┘
```

1. `RuntimeState` spawns the agent subprocess, performs the ACP `initialize` / `newSession` handshake, and loads the theme.
2. `App` owns two screens — the conversation and a git diff viewer — plus a settings overlay. It routes terminal events, ACP events, and ticks to the active screen.
3. The `tui` library's diff-based `Renderer` turns each frame into minimal ANSI output.

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

## Slash commands

Type `/` in the input to open the command picker. Built-in commands:

| Command | Description |
|---------|-------------|
| `/clear` | Clear screen and start a new session |
| `/settings` | Open settings overlay |
| `/resume` | Resume a previous session |

Additional commands may be available from the agent (e.g., `/search`, `/web`).

## Settings

Wisp has two kinds of settings:

1. **Wisp settings** (`~/.wisp/settings.json`) — UI preferences like themes
2. **Agent settings** — Model, reasoning effort, MCP servers, etc. These come from the agent and are configured in-app via `/settings`

Override the Wisp home directory with `WISP_HOME` environment variable.

### Themes

Place `.tmTheme` files in `~/.wisp/themes/`:

```bash
mkdir -p ~/.wisp/themes
cp my-theme.tmTheme ~/.wisp/themes/
```

Then set in `~/.wisp/settings.json`:

```json
{
  "theme": { "file": "my-theme.tmTheme" }
}
```

Remove `"file"` or set it to `null` to use the default theme.

## Logs

Debug logs are written to `/tmp/wisp-logs/wisp.log.YYYY-MM-DD` by default. Override with:

```bash
wisp --log-dir ~/logs
```

## Documentation

Run `cargo doc -p wisp --open` for full API docs. Key entry points:

- `run_tui` — launch wisp with an agent command
- `RuntimeState` — ACP session bootstrap
- `App` — main application component
- `settings` — Wisp and theme configuration

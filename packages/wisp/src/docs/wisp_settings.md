Persisted UI preferences for Wisp.

Serialized as JSON to `~/.wisp/settings.json` (or `$WISP_HOME/settings.json`). Currently holds only theme configuration via [`ThemeSettings`].

# JSON shape

```json
{
  "theme": {
    "file": "catppuccin.tmTheme"
  }
}
```

Set `"file"` to `null` or omit it to use the default theme. The filename must be a bare basename pointing to a `.tmTheme` file in `~/.wisp/themes/`.

# See also

- [`load_or_create_settings`](crate::settings::load_or_create_settings) — reads or creates this file
- [`load_theme`](crate::settings::load_theme) — resolves the theme from these settings

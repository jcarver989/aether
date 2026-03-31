A multi-field form rendered as a tabbed pane with a virtual "Submit" tab.

Each [`FormField`] gets its own full pane. A tab bar at the top shows all fields plus a final Submit tab. Navigate between tabs with Tab/BackTab or arrow keys (for non-text fields). The active field's widget receives all other input.

# Construction

```rust,no_run
use tui::{Form, FormField, FormFieldKind, TextField, SelectOption, RadioSelect};

let form = Form::new("Configure project".to_string(), vec![
    FormField {
        name: "name".to_string(),
        label: "Project name".to_string(),
        description: Some("The directory name for your project".to_string()),
        required: true,
        kind: FormFieldKind::Text(TextField::new(String::new())),
    },
]);
```

# Messages

`Form` implements [`Component`](crate::Component) with `Message = FormMessage`:

- **`FormMessage::Close`** — Emitted on Esc.
- **`FormMessage::Submit`** — Emitted on Enter while the Submit tab is focused.

# Serialization

[`to_json()`](Form::to_json) serializes all field values to a `serde_json::Value` object, keyed by each field's `name`.

# `FormField`

A single field within the form:

- **`name`** — Machine-readable key (used in JSON output).
- **`label`** — Human-readable label shown in the tab bar and pane header.
- **`description`** — Optional help text shown below the field.
- **`required`** — If `true`, an asterisk is shown next to the label.
- **`kind`** — The backing widget, as a [`FormFieldKind`].

# `FormFieldKind`

- **`Text(TextField)`** — Single-line text input.
- **`Number(NumberField)`** — Numeric input.
- **`Boolean(Checkbox)`** — Toggle rendered as `[x]` / `[ ]`.
- **`SingleSelect(RadioSelect)`** — Radio button list.
- **`MultiSelect(MultiSelect)`** — Checkbox list.

# See also

- [`TextField`](crate::TextField) — Text input widget.
- [`Checkbox`](crate::Checkbox) — Boolean toggle.
- [`RadioSelect`](crate::RadioSelect) — Single-select list.
- [`MultiSelect`](crate::MultiSelect) — Multi-select list.
- [`FocusRing`](crate::FocusRing) — Used internally for tab navigation.

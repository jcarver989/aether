# Plan: @-file Include Support for Wisp

## Overview

This feature allows users to reference local files in their prompts using `@filename` syntax. When triggered:
1. User types `@` in the input prompt
2. A fuzzy-matched file picker appears below the input
3. User selects a file with arrow keys + Enter
4. The file content is embedded as an ACP `ContentBlock::Resource` and sent with the prompt

Based on the [ACP Embedded Resources](https://agentclientprotocol.com/protocol/content#embedded-resources) spec, the embedded file becomes:

```json
{
  "type": "resource",
  "resource": {
    "uri": "file:///path/to/file.rs",
    "mimeType": "text/x-rust",
    "text": "file contents here..."
  }
}
```

---

## 1. Detect `@` Trigger in Input

**Location**: `src/renderer.rs` - modify `on_key_event()`

### Implementation

Add new state to `Renderer`:

```rust
pub struct Renderer<T: Write> {
    // ... existing fields
    pub file_picker: Option<FilePicker>,  // NEW
}

pub struct FilePicker {
    pub query: String,           // Text after @ (e.g., "src/main")
    pub files: Vec<FileMatch>,   // Fuzzy-matched files
    pub selected_index: usize,   // Current selection
}

pub struct FileMatch {
    pub path: PathBuf,
    pub display_name: String,    // Shortened for display
    pub score: f64,              // Fuzzy match score
}
```

### Logic

- When user types `@`: Enter "file picker mode", trigger initial file scan
- When user types more characters: Update query, re-run fuzzy matching

---

## 2. File Discovery & Fuzzy Matching

**Location**: New file `src/components/file_picker.rs`

### File Search Strategy

1. **Root**: Use current working directory (`std::env::current_dir()`)
2. **Walk**: Use `walkdir` crate (check if available)
3. **Filter**: Exclude hidden files, `.git/`, `node_modules/`, `target/`
4. **Limits**: Max 50 files shown, max 1000 scanned

### Fuzzy Matching

Use `fuzzy-matcher` crate:

```rust
use fuzzy_matcher::skim::SkimMatcherV2;

let matcher = SkimMatcherV2::default();
if let Some((score, _)) = matcher.fuzzy_match(&filename, query) {
    // Add to results
}
```

---

## 3. File Picker UI Component

**Location**: `src/components/file_picker.rs`

### Rendering

Appears below input prompt:

```
╭────────────────────────────────────────────────────╮
│ │ > @src/ma                                         │
╰────────────────────────────────────────────────────╯
  ▶  src/main.rs           (best match)
    src/main_test.rs
    src/manifest.rs
```

### Component Structure

```rust
pub struct FilePickerComponent<'a> {
    pub picker: &'a FilePicker,
    pub input_cursor_pos: usize,
}

impl Component for FilePickerComponent<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        // Selected item: "▶" prefix
        // Other items: indented
        // Truncate long paths
    }
}
```

---

## 4. Keyboard Navigation

**Location**: `src/renderer.rs` - modify `on_key_event()`

| Key | Action |
|-----|--------|
| `Up` / `Ctrl+P` | Selection up |
| `Down` / `Ctrl+N` | Selection down |
| `Enter` | Confirm selection |
| `Escape` | Cancel picker |
| `Backspace` | Update query or exit |
| `Tab` | Auto-complete |

---

## 5. File Reading & Content Embedding

**Location**: New method in `src/renderer.rs`

```rust
fn confirm_file_selection(&mut self) -> Result<(), WispError> {
    let picker = self.file_picker.take().unwrap();
    
    if let Some(selected) = picker.files.get(picker.selected_index) {
        let content = std::fs::read_to_string(&selected.path)?;
        let mime_type = mime_guess::from_path(&selected.path)
            .first_or_octet_stream()
            .to_string();
        let uri = format!("file://{}", selected.path.display());
        
        self.pending_embedded_files.push(EmbeddedFile {
            uri,
            mime_type,
            text: content,
        });
    }
    Ok(())
}
```

### Multiple File Support

```
@file1 @file2 Explain the difference
```

---

## 6. ACP Content Block Integration

**Location**: Where `prompt_handle.prompt()` is called

```rust
if !self.pending_embedded_files.is_empty() {
    let content_blocks: Vec<acp::ContentBlock> = 
        self.pending_embedded_files
            .iter()
            .map(|file| {
                acp::ContentBlock::Resource(acp::ContentBlockResource {
                    resource: acp::EmbeddedResourceResource::Text(
                        acp::EmbeddedResourceText {
                            uri: file.uri.clone(),
                            mime_type: Some(file.mime_type.clone()),
                            text: file.text.clone(),
                        }
                    ),
                })
            })
            .collect();
    
    prompt_handle.prompt_with_content(session_id, &user_input, content_blocks)?;
    self.pending_embedded_files.clear();
} else {
    prompt_handle.prompt(session_id, &user_input)?;
}
```

---

## 7. Display @ Mentions in Input

**Location**: `src/components/input_prompt.rs`

Render `@filename` with special styling:

```
╭────────────────────────────────────────────────────╮
│ │ > @main.rs @utils.rs analyze these files        │
╰────────────────────────────────────────────────────╯
```

---

## 8. Cursor Positioning

**Location**: `src/renderer.rs` - modify `position_cursor_in_input()`

When picker is active, position cursor at end of `@query`:

```rust
if let Some(ref picker) = self.file_picker {
    let at_pos = self.input_buffer.rfind('@').unwrap_or(0);
    let col = 4 + at_pos + 1 + picker.query.len() as u16;
    self.tui.reposition_cursor(2, col)
}
```

---

## 9. Edge Cases

| Case | Handling |
|------|----------|
| File not found | Show error, don't embed |
| Binary file | Warn user, skip or truncate |
| Large file (>1MB) | Truncate with warning |
| Permission denied | Show error, continue |
| No matches | Show "No files found" |
| Escape | Cancel, remove `@` from input |

---

## 10. Dependencies to Add

```toml
# Cargo.toml
fuzzy-matcher = "0.3"
mime_guess = "2"
# walkdir likely already available
```

---

## Implementation Order

1. **Phase 1**: Trigger detection + empty picker display
2. **Phase 2**: File discovery + fuzzy matching
3. **Phase 3**: Picker UI + keyboard navigation
4. **Phase 4**: File reading + embedding
5. **Phase 5**: ACP integration
6. **Phase 6**: Edge cases + polish

---

## Files to Modify

| File | Action |
|------|--------|
| `src/components/file_picker.rs` | Create |
| `src/components/mod.rs` | Export new component |
| `src/renderer.rs` | Add state, keyboard handling |
| `src/components/input_prompt.rs` | Render @ mentions |
| `Cargo.toml` | Add dependencies |

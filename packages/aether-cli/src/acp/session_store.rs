use aether_core::context::ext::{SessionEvent, UserEvent};
use aether_core::events::AgentMessage;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub session_id: String,
    pub cwd: PathBuf,
    pub model: String,
    #[serde(default)]
    pub selected_mode: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub meta: SessionMeta,
    pub title: Option<String>,
}

pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    pub fn new() -> io::Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Home directory not found"))?;
        Ok(Self {
            dir: home.join(".aether/sessions"),
        })
    }

    #[cfg(test)]
    pub fn from_path(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn append_meta(&self, session_id: &str, meta: &SessionMeta) -> io::Result<()> {
        self.append_line(session_id, meta)
    }

    pub fn append_event(&self, session_id: &str, event: &SessionEvent) -> io::Result<()> {
        if is_streaming_event(event) {
            return Ok(());
        }
        self.append_line(session_id, event)
    }

    pub fn load(&self, session_id: &str) -> Option<(SessionMeta, Vec<SessionEvent>)> {
        let path = self.session_path(session_id);
        let file = File::open(&path).ok()?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let meta_line = lines.next()?.ok()?;
        let meta: SessionMeta = match serde_json::from_str(&meta_line) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to parse session meta: {e}");
                return None;
            }
        };

        let mut events = Vec::new();
        for line in lines {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionEvent>(&line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    warn!("Skipping malformed session log line: {e}");
                }
            }
        }

        Some((meta, events))
    }

    pub fn list(&self) -> Vec<SessionSummary> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };

        let mut sessions: Vec<SessionSummary> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    return None;
                }
                let file = File::open(&path).ok()?;
                let mut reader = BufReader::new(file);

                let mut first_line = String::new();
                reader.read_line(&mut first_line).ok()?;
                let meta = serde_json::from_str::<SessionMeta>(first_line.trim()).ok()?;

                let mut second_line = String::new();
                let title = reader
                    .read_line(&mut second_line)
                    .ok()
                    .and_then(|n| (n > 0).then_some(()))
                    .and_then(|()| serde_json::from_str::<SessionEvent>(second_line.trim()).ok())
                    .and_then(|event| match event {
                        SessionEvent::User(UserEvent::Message { content }) => {
                            Some(extract_title(&content))
                        }
                        _ => None,
                    });

                Some(SessionSummary { meta, title })
            })
            .collect();

        sessions.sort_by(|a, b| b.meta.created_at.cmp(&a.meta.created_at));
        sessions
    }

    fn append_line<T: Serialize>(&self, session_id: &str, value: &T) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.session_path(session_id);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        let line = serde_json::to_string(value)
            .map_err(|e| io::Error::other(format!("Failed to serialize log entry: {e}")))?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.dir.join(format!("{session_id}.jsonl"))
    }
}

const MAX_TITLE_LEN: usize = 80;

fn extract_title(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or("").trim();
    if first_line.len() > MAX_TITLE_LEN {
        let end = first_line.floor_char_boundary(MAX_TITLE_LEN);
        format!("{}…", &first_line[..end])
    } else {
        first_line.to_string()
    }
}

fn is_streaming_event(event: &SessionEvent) -> bool {
    matches!(
        event,
        SessionEvent::Agent(
            AgentMessage::Text {
                is_complete: false,
                ..
            } | AgentMessage::Thought {
                is_complete: false,
                ..
            }
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::context::ext::UserEvent;
    use llm::ToolCallResult;

    fn meta(id: &str, created_at: &str, mode: Option<&str>) -> SessionMeta {
        SessionMeta {
            session_id: id.to_string(),
            cwd: PathBuf::from("/tmp"),
            model: "test-model".to_string(),
            selected_mode: mode.map(str::to_string),
            created_at: created_at.to_string(),
        }
    }

    fn default_meta() -> SessionMeta {
        meta("s1", "2026-01-01T00:00:00Z", Some("planner"))
    }

    fn user_msg(content: &str) -> SessionEvent {
        SessionEvent::User(UserEvent::Message {
            content: content.to_string(),
        })
    }

    fn agent_text(msg_id: &str, chunk: &str, complete: bool) -> SessionEvent {
        SessionEvent::Agent(AgentMessage::Text {
            message_id: msg_id.to_string(),
            chunk: chunk.to_string(),
            is_complete: complete,
            model_name: "test".to_string(),
        })
    }

    /// Create a temp dir and store; returns both so the dir lives long enough.
    fn temp_store() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());
        (dir, store)
    }

    /// Append default meta + an optional user message, return listed title.
    fn listed_title(content: Option<&str>) -> Option<String> {
        let (_dir, store) = temp_store();
        store.append_meta("s1", &default_meta()).unwrap();
        if let Some(c) = content {
            store.append_event("s1", &user_msg(c)).unwrap();
        }
        store.list().into_iter().next().unwrap().title
    }

    #[test]
    fn append_meta_persists_selected_mode_field() {
        let (dir, store) = temp_store();
        store.append_meta("s1", &default_meta()).unwrap();
        let raw = std::fs::read_to_string(dir.path().join("s1.jsonl")).unwrap();
        assert!(
            raw.contains("\"selectedMode\""),
            "missing selectedMode: {raw}"
        );
    }

    #[test]
    fn append_and_load_roundtrip() {
        let (_dir, store) = temp_store();
        let m = default_meta();
        let user = user_msg("Hello");
        let agent = agent_text("msg_1", "Hi there!", true);

        store.append_meta("s1", &m).unwrap();
        store.append_event("s1", &user).unwrap();
        store.append_event("s1", &agent).unwrap();

        let (loaded, events) = store.load("s1").unwrap();
        assert_eq!(loaded, m);
        assert_eq!(events, vec![user, agent]);
    }

    #[test]
    fn load_skips_malformed_trailing_line() {
        let (dir, store) = temp_store();
        let m = default_meta();
        let mut file = File::create(dir.path().join("s2.jsonl")).unwrap();
        writeln!(file, "{}", serde_json::to_string(&m).unwrap()).unwrap();
        writeln!(file, "{{truncated garbage").unwrap();

        let (loaded, events) = store.load("s2").unwrap();
        assert_eq!(loaded, m);
        assert!(events.is_empty());
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let (_dir, store) = temp_store();
        assert!(store.load("nonexistent").is_none());
    }

    #[test]
    fn append_drops_streaming_chunks_and_persists_everything_else() {
        let (_dir, store) = temp_store();
        store.append_meta("s1", &default_meta()).unwrap();

        let dropped = [
            agent_text("m", "partial", false),
            SessionEvent::Agent(AgentMessage::Thought {
                message_id: "m".to_string(),
                chunk: "thinking".to_string(),
                is_complete: false,
                model_name: "test".to_string(),
            }),
        ];
        let kept = vec![
            agent_text("m", "full", true),
            SessionEvent::Agent(AgentMessage::Error {
                message: "oops".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::Done),
            SessionEvent::Agent(AgentMessage::ToolResult {
                result: ToolCallResult {
                    id: "1".to_string(),
                    name: "t".to_string(),
                    arguments: "{}".to_string(),
                    result: "ok".to_string(),
                },
                result_meta: None,
                model_name: "test".to_string(),
            }),
            SessionEvent::Agent(AgentMessage::ToolCallUpdate {
                tool_call_id: "1".to_string(),
                chunk: r#"{"filePath":"Cargo.toml"}"#.to_string(),
                model_name: "test".to_string(),
            }),
        ];

        for e in &dropped {
            store.append_event("s1", e).unwrap();
        }
        for e in &kept {
            store.append_event("s1", e).unwrap();
        }

        let (_, events) = store.load("s1").unwrap();
        assert_eq!(events, kept);
    }

    #[test]
    fn list_empty_and_nonexistent_dirs_return_empty() {
        let (_dir, store) = temp_store();
        assert!(store.list().is_empty());

        let missing = SessionStore::from_path(PathBuf::from("/nonexistent/path"));
        assert!(missing.list().is_empty());
    }

    #[test]
    fn list_returns_sessions_sorted_by_created_at_descending() {
        let (_dir, store) = temp_store();
        let old = meta("s-old", "2026-01-01T00:00:00Z", None);
        let new = meta("s-new", "2026-02-01T00:00:00Z", None);
        store.append_meta("s-old", &old).unwrap();
        store.append_meta("s-new", &new).unwrap();

        let ids: Vec<_> = store
            .list()
            .iter()
            .map(|s| s.meta.session_id.clone())
            .collect();
        assert_eq!(ids, vec!["s-new", "s-old"]);
    }

    #[test]
    fn list_skips_malformed_files() {
        let (dir, store) = temp_store();
        store.append_meta("s1", &default_meta()).unwrap();
        std::fs::write(dir.path().join("bad.jsonl"), "not valid json\n").unwrap();
        std::fs::write(dir.path().join("readme.txt"), "ignore me").unwrap();

        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].meta.session_id, "s1");
    }

    #[test]
    fn list_title_extraction() {
        let cases: &[(&str, Option<&str>)] = &[
            ("Fix the login bug", Some("Fix the login bug")),
            ("First line\nSecond\nThird", Some("First line")),
        ];
        for (input, expected) in cases {
            assert_eq!(
                listed_title(Some(input)).as_deref(),
                *expected,
                "input: {input}"
            );
        }
    }

    #[test]
    fn list_returns_none_title_when_no_user_message() {
        assert_eq!(listed_title(None), None);
    }

    #[test]
    fn list_truncates_long_title() {
        let title = listed_title(Some(&"a".repeat(120))).unwrap();
        assert!(title.len() <= 84); // 80 chars + "…" (3 bytes)
        assert!(title.ends_with('…'));
    }
}

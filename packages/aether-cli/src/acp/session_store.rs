use aether_core::context::ext::SessionEvent;
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

    pub fn list(&self) -> Vec<SessionMeta> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };

        let mut sessions: Vec<SessionMeta> = entries
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
                serde_json::from_str::<SessionMeta>(first_line.trim()).ok()
            })
            .collect();

        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
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

    fn test_meta() -> SessionMeta {
        SessionMeta {
            session_id: "s1".to_string(),
            cwd: PathBuf::from("/tmp"),
            model: "test-model".to_string(),
            selected_mode: Some("planner".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn append_meta_persists_selected_mode_field() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());

        let meta = test_meta();
        store.append_meta("s1", &meta).unwrap();

        let raw = std::fs::read_to_string(dir.path().join("s1.jsonl")).unwrap();
        assert!(
            raw.contains("\"selectedMode\""),
            "expected selectedMode field in serialized session meta: {raw}"
        );
    }

    #[test]
    fn append_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());

        let meta = test_meta();
        let user = SessionEvent::User(UserEvent::Message {
            content: "Hello".to_string(),
        });
        let agent = SessionEvent::Agent(AgentMessage::Text {
            message_id: "msg_1".to_string(),
            chunk: "Hi there!".to_string(),
            is_complete: true,
            model_name: "test".to_string(),
        });

        store.append_meta("s1", &meta).unwrap();
        store.append_event("s1", &user).unwrap();
        store.append_event("s1", &agent).unwrap();

        let (loaded_meta, events) = store.load("s1").unwrap();
        assert_eq!(loaded_meta, meta);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], user);
        assert_eq!(events[1], agent);
    }

    #[test]
    fn load_skips_malformed_trailing_line() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());
        let path = dir.path().join("s2.jsonl");

        let meta = test_meta();
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", serde_json::to_string(&meta).unwrap()).unwrap();
        writeln!(file, "{{truncated garbage").unwrap();

        let (loaded_meta, events) = store.load("s2").unwrap();
        assert_eq!(loaded_meta, meta);
        assert!(events.is_empty());
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());
        assert!(store.load("nonexistent").is_none());
    }

    #[test]
    fn append_drops_streaming_chunks_and_persists_everything_else() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());

        store.append_meta("s1", &test_meta()).unwrap();

        let streaming_text = SessionEvent::Agent(AgentMessage::Text {
            message_id: "m".to_string(),
            chunk: "partial".to_string(),
            is_complete: false,
            model_name: "test".to_string(),
        });
        let streaming_thought = SessionEvent::Agent(AgentMessage::Thought {
            message_id: "m".to_string(),
            chunk: "thinking".to_string(),
            is_complete: false,
            model_name: "test".to_string(),
        });
        let complete_text = SessionEvent::Agent(AgentMessage::Text {
            message_id: "m".to_string(),
            chunk: "full".to_string(),
            is_complete: true,
            model_name: "test".to_string(),
        });
        let error = SessionEvent::Agent(AgentMessage::Error {
            message: "oops".to_string(),
        });
        let done = SessionEvent::Agent(AgentMessage::Done);
        let tool_result = SessionEvent::Agent(AgentMessage::ToolResult {
            result: ToolCallResult {
                id: "1".to_string(),
                name: "t".to_string(),
                arguments: "{}".to_string(),
                result: "ok".to_string(),
            },
            result_meta: None,
            model_name: "test".to_string(),
        });

        // Streaming chunks should be silently dropped
        store.append_event("s1", &streaming_text).unwrap();
        store.append_event("s1", &streaming_thought).unwrap();

        // Everything else should persist
        store.append_event("s1", &complete_text).unwrap();
        store.append_event("s1", &error).unwrap();
        store.append_event("s1", &done).unwrap();
        store.append_event("s1", &tool_result).unwrap();

        let (_, events) = store.load("s1").unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0], complete_text);
        assert_eq!(events[1], error);
        assert_eq!(events[2], done);
        assert_eq!(events[3], tool_result);
    }

    #[test]
    fn list_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());
        assert!(store.list().is_empty());
    }

    #[test]
    fn list_nonexistent_dir_returns_empty() {
        let store = SessionStore::from_path(PathBuf::from("/nonexistent/path"));
        assert!(store.list().is_empty());
    }

    #[test]
    fn list_returns_sessions_sorted_by_created_at_descending() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());

        let meta_old = SessionMeta {
            session_id: "s-old".to_string(),
            cwd: PathBuf::from("/tmp"),
            model: "test-model".to_string(),
            selected_mode: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let meta_new = SessionMeta {
            session_id: "s-new".to_string(),
            cwd: PathBuf::from("/tmp"),
            model: "test-model".to_string(),
            selected_mode: None,
            created_at: "2026-02-01T00:00:00Z".to_string(),
        };

        store.append_meta("s-old", &meta_old).unwrap();
        store.append_meta("s-new", &meta_new).unwrap();

        let listed = store.list();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].session_id, "s-new");
        assert_eq!(listed[1].session_id, "s-old");
    }

    #[test]
    fn list_skips_malformed_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().to_path_buf());

        store.append_meta("s1", &test_meta()).unwrap();

        // Write a malformed JSONL file
        std::fs::write(dir.path().join("bad.jsonl"), "not valid json\n").unwrap();

        // Write a non-jsonl file
        std::fs::write(dir.path().join("readme.txt"), "ignore me").unwrap();

        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].session_id, "s1");
    }
}

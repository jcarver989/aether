use crate::storage::{EvalResult, Result, ResultsStore, TraceEvent};
use std::collections::HashMap;
use std::{fs, io::BufRead, path::PathBuf};
use tracing_subscriber::{Layer, Registry, fmt};
use uuid::Uuid;

/// File system-based implementation of `ResultsStore`
#[derive(Clone)]
pub struct FileSystemStore {
    output_dir: PathBuf,
}

impl FileSystemStore {
    pub fn new(output_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&output_dir)?;
        fs::create_dir_all(output_dir.join("runs"))?;
        Ok(Self { output_dir })
    }

    fn run_dir(&self, run_id: Uuid) -> PathBuf {
        self.output_dir.join("runs").join(run_id.to_string())
    }

    fn traces_file(&self, run_id: Uuid) -> PathBuf {
        self.run_dir(run_id).join("traces.jsonl")
    }

    fn result_file(&self, run_id: Uuid, eval_id: Uuid) -> PathBuf {
        self.run_dir(run_id)
            .join("results")
            .join(format!("{eval_id}.json"))
    }

    fn results_dir(&self, run_id: Uuid) -> PathBuf {
        self.run_dir(run_id).join("results")
    }

    /// Parse traces from JSONL file
    fn parse_traces_file(&self, path: &PathBuf) -> Result<Vec<TraceEvent>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<TraceEvent>(&line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    tracing::warn!("Failed to parse trace event: {}", e);
                }
            }
        }

        Ok(events)
    }

    /// Group traces by eval ID using span hierarchy
    fn group_traces_by_eval(
        &self,
        events: Vec<TraceEvent>,
    ) -> Result<HashMap<String, Vec<TraceEvent>>> {
        let mut grouped: HashMap<String, Vec<TraceEvent>> = HashMap::new();

        // Group events by eval_id found in current span or parent spans
        for event in events {
            // Try to find eval_id from current span first
            let eval_id = if let Some(span_info) = &event.span {
                span_info
                    .extra
                    .get("eval_id")
                    .and_then(|v| v.as_str())
                    .map(std::string::ToString::to_string)
            } else {
                None
            };

            // If not found in current span, check parent spans
            let eval_id = eval_id.or_else(|| {
                event.spans.iter().find_map(|parent_span| {
                    parent_span
                        .extra
                        .get("eval_id")
                        .and_then(|v| v.as_str())
                        .map(std::string::ToString::to_string)
                })
            });

            if let Some(eval_id) = eval_id {
                grouped.entry(eval_id).or_default().push(event);
            } else {
                // Ungrouped events
                grouped
                    .entry("_ungrouped".to_string())
                    .or_default()
                    .push(event);
            }
        }

        tracing::debug!("Grouped into {} groups", grouped.len());
        Ok(grouped)
    }
}

impl ResultsStore for FileSystemStore {
    async fn get_run_ids(&self) -> Result<Vec<Uuid>> {
        let runs_dir = self.output_dir.join("runs");
        let mut run_ids = Vec::new();

        if let Ok(entries) = fs::read_dir(&runs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && let Some(dir_name) = path.file_name().and_then(|n| n.to_str())
                    && let Ok(uuid) = Uuid::parse_str(dir_name)
                {
                    run_ids.push(uuid);
                }
            }
        }

        // Sort by directory modification time (most recent first)
        run_ids.sort_by(|a, b| {
            let a_modified = fs::metadata(self.run_dir(*a))
                .and_then(|m| m.modified())
                .ok();
            let b_modified = fs::metadata(self.run_dir(*b))
                .and_then(|m| m.modified())
                .ok();
            b_modified.cmp(&a_modified)
        });

        Ok(run_ids)
    }

    async fn save_eval_result(&self, run_id: Uuid, report: &EvalResult) -> Result<()> {
        let result_file = self.result_file(run_id, report.id());
        if let Some(parent) = result_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(report).map_err(Box::new)?;
        fs::write(result_file, json).map_err(Box::new)?;
        Ok(())
    }

    async fn get_eval_results(&self, run_id: Uuid) -> Result<Vec<EvalResult>> {
        let results_dir = self.results_dir(run_id);
        fs::create_dir_all(&results_dir)?;
        let mut results = Vec::new();

        if let Ok(entries) = fs::read_dir(&results_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let result = fs::read_to_string(&path).and_then(|json| {
                        serde_json::from_str::<EvalResult>(&json)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                    });

                    match result {
                        Ok(eval_result) => results.push(eval_result),
                        Err(e) => tracing::warn!(
                            "Failed to read/parse eval result file {:?}: {}",
                            path,
                            e
                        ),
                    }
                }
            }
        }

        Ok(results)
    }

    async fn get_eval_result(&self, run_id: Uuid, eval_id: Uuid) -> Result<Option<EvalResult>> {
        let result_file = self.result_file(run_id, eval_id);

        if !result_file.exists() {
            return Ok(None);
        }

        match fs::read_to_string(&result_file) {
            Ok(json) => match serde_json::from_str::<EvalResult>(&json) {
                Ok(eval_result) => Ok(Some(eval_result)),
                Err(e) => {
                    tracing::warn!("Failed to parse eval result file {:?}: {}", result_file, e);
                    Ok(None)
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read eval result file {:?}: {}", result_file, e);
                Ok(None)
            }
        }
    }

    async fn get_eval_traces(&self, run_id: Uuid, eval_id: Uuid) -> Result<Vec<TraceEvent>> {
        let traces_file = self.traces_file(run_id);
        let all_events = self.parse_traces_file(&traces_file)?;
        tracing::debug!("Parsed {} total trace events", all_events.len());

        let grouped = self.group_traces_by_eval(all_events)?;
        tracing::debug!(
            "Grouped into {} groups: {:?}",
            grouped.len(),
            grouped.keys().collect::<Vec<_>>()
        );

        // Use eval_id as the key (converted to string)
        let eval_id_str = eval_id.to_string();
        tracing::debug!("Looking for eval_id: {}", eval_id_str);

        let traces = grouped.get(&eval_id_str).cloned().unwrap_or_default();
        tracing::debug!("Found {} traces for eval_id {}", traces.len(), eval_id_str);

        Ok(traces)
    }

    fn create_tracing_layer(&self, run_id: Uuid) -> Box<dyn Layer<Registry> + Send + Sync> {
        let run_dir = self.run_dir(run_id);
        fs::create_dir_all(&run_dir).expect("Failed to create run directory");

        let traces_file = self.traces_file(run_id);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(traces_file)
            .expect("Failed to open traces file");

        Box::new(
            fmt::layer()
                .json()
                .with_writer(move || file.try_clone().expect("Failed to clone file handle")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SpanInfo;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_deserialize_span_with_eval_id() {
        // Test that SpanInfo correctly deserializes eval_id from top level
        let json = r#"{"eval_id":"09373995-4265-4d6d-9315-26914861a182","eval_name":"test","name":"eval_task"}"#;
        let span: SpanInfo = serde_json::from_str(json).unwrap();

        assert_eq!(span.name, "eval_task");
        assert!(span.extra.get("eval_id").is_some());
        assert_eq!(
            span.extra.get("eval_id").and_then(|v| v.as_str()),
            Some("09373995-4265-4d6d-9315-26914861a182")
        );
    }

    #[test]
    fn test_parse_traces_file_empty() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSystemStore::new(temp_dir.path().to_path_buf()).unwrap();
        let traces_file = temp_dir.path().join("empty.jsonl");

        let result = store.parse_traces_file(&traces_file).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_traces_file_valid_events() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSystemStore::new(temp_dir.path().to_path_buf()).unwrap();
        let traces_file = temp_dir.path().join("traces.jsonl");

        // Write sample trace events
        let mut file = fs::File::create(&traces_file).unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-01T12:00:00Z","level":"INFO","target":"test","fields":{{"message":"test event"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-01T12:00:01Z","level":"DEBUG","target":"test","fields":{{"message":"debug event"}}}}"#
        )
        .unwrap();

        let result = store.parse_traces_file(&traces_file).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].level, "INFO");
        assert_eq!(result[1].level, "DEBUG");
    }

    #[test]
    fn test_group_traces_by_eval() {
        let store = FileSystemStore::new(TempDir::new().unwrap().path().to_path_buf()).unwrap();

        let eval_id_1 = "11111111-1111-1111-1111-111111111111";
        let eval_id_2 = "22222222-2222-2222-2222-222222222222";

        let events = vec![
            TraceEvent {
                timestamp: "2024-01-01T12:00:00Z".to_string(),
                level: "INFO".to_string(),
                target: "test".to_string(),
                fields: serde_json::json!({"message": "eval1 event"}),
                span: Some(SpanInfo {
                    id: Some(1),
                    name: "eval_span".to_string(),
                    fields: None,
                    extra: serde_json::json!({"eval_id": eval_id_1}),
                }),
                spans: vec![],
            },
            TraceEvent {
                timestamp: "2024-01-01T12:00:01Z".to_string(),
                level: "INFO".to_string(),
                target: "test".to_string(),
                fields: serde_json::json!({"message": "eval2 event"}),
                span: Some(SpanInfo {
                    id: Some(2),
                    name: "eval_span".to_string(),
                    fields: None,
                    extra: serde_json::json!({"eval_id": eval_id_2}),
                }),
                spans: vec![],
            },
            TraceEvent {
                timestamp: "2024-01-01T12:00:02Z".to_string(),
                level: "INFO".to_string(),
                target: "test".to_string(),
                fields: serde_json::json!({"message": "ungrouped event"}),
                span: None,
                spans: vec![],
            },
        ];

        let grouped = store.group_traces_by_eval(events).unwrap();

        assert_eq!(grouped.len(), 3);
        assert!(grouped.contains_key(eval_id_1));
        assert!(grouped.contains_key(eval_id_2));
        assert!(grouped.contains_key("_ungrouped"));
        assert_eq!(grouped.get(eval_id_1).unwrap().len(), 1);
        assert_eq!(grouped.get(eval_id_2).unwrap().len(), 1);
        assert_eq!(grouped.get("_ungrouped").unwrap().len(), 1);
    }
}

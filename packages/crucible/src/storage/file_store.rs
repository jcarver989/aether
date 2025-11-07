use crate::storage::{EvalResult, Result, ResultsStore, TraceEvent};
use std::collections::HashMap;
use std::{fs, io::BufRead, path::PathBuf};
use tracing_subscriber::{Layer, Registry, fmt};
use uuid::Uuid;

/// File system-based implementation of ResultsStore
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

    fn result_file(&self, run_id: Uuid, eval_name: &str) -> PathBuf {
        self.run_dir(run_id)
            .join("results")
            .join(format!("{}.json", eval_name))
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

    /// Group traces by eval name using span hierarchy
    fn group_traces_by_eval(
        &self,
        events: Vec<TraceEvent>,
    ) -> Result<HashMap<String, Vec<TraceEvent>>> {
        let mut grouped: HashMap<String, Vec<TraceEvent>> = HashMap::new();
        let mut span_to_eval: HashMap<u64, String> = HashMap::new();

        // First pass: build span -> eval name mapping
        for event in &events {
            // Check if this is an eval span (has eval_name field)
            if let Some(span_info) = &event.span {
                if let Some(fields) = &span_info.fields {
                    if let Some(eval_name) = fields.get("eval_name").and_then(|v| v.as_str()) {
                        if let Some(span_id) = span_info.id {
                            span_to_eval.insert(span_id, eval_name.to_string());
                        }
                    }
                }
            }
        }

        // Second pass: group events by eval
        for event in events {
            // Try to find eval name from current span or parent spans
            let eval_name = if let Some(span_info) = &event.span {
                // Check current span
                if let Some(id) = span_info.id {
                    if let Some(name) = span_to_eval.get(&id) {
                        Some(name.clone())
                    } else {
                        // Check parent spans
                        event.spans.iter().find_map(|parent| {
                            parent.id.and_then(|id| span_to_eval.get(&id).cloned())
                        })
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(eval_name) = eval_name {
                grouped.entry(eval_name).or_default().push(event);
            } else {
                // Ungrouped events
                grouped
                    .entry("_ungrouped".to_string())
                    .or_default()
                    .push(event);
            }
        }

        Ok(grouped)
    }
}

impl ResultsStore for FileSystemStore {
    async fn save_eval_result(
        &self,
        run_id: Uuid,
        eval_name: &str,
        report: &EvalResult,
    ) -> Result<()> {
        let result_file = self.result_file(run_id, eval_name);
        if let Some(parent) = result_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(report).map_err(|e| Box::new(e))?;
        fs::write(result_file, json).map_err(|e| Box::new(e))?;
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

    async fn get_eval_traces(&self, run_id: Uuid, eval_name: &str) -> Result<Vec<TraceEvent>> {
        let traces_file = self.traces_file(run_id);
        let all_events = self.parse_traces_file(&traces_file)?;
        let grouped = self.group_traces_by_eval(all_events)?;

        Ok(grouped.get(eval_name).cloned().unwrap_or_default())
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
    use crate::storage::{EvalAssertionResult, EvalReport, SpanInfo};
    use std::io::Write;
    use tempfile::TempDir;

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

        let events = vec![
            TraceEvent {
                timestamp: "2024-01-01T12:00:00Z".to_string(),
                level: "INFO".to_string(),
                target: "test".to_string(),
                fields: serde_json::json!({"message": "eval1 event"}),
                span: Some(SpanInfo {
                    id: Some(1),
                    name: "eval_span".to_string(),
                    fields: Some(serde_json::json!({"eval_name": "test_eval_1"})),
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
                    fields: Some(serde_json::json!({"eval_name": "test_eval_2"})),
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
        assert!(grouped.contains_key("test_eval_1"));
        assert!(grouped.contains_key("test_eval_2"));
        assert!(grouped.contains_key("_ungrouped"));
        assert_eq!(grouped.get("test_eval_1").unwrap().len(), 1);
        assert_eq!(grouped.get("test_eval_2").unwrap().len(), 1);
        assert_eq!(grouped.get("_ungrouped").unwrap().len(), 1);
    }

    #[test]
    fn test_eval_report_computed_methods() {
        use chrono::Utc;

        let mut report = EvalReport::new(uuid::Uuid::new_v4(), Utc::now(), Some(5), Some(1000));

        // Add some eval results
        report.add_eval_result(EvalResult {
            eval_name: "eval1".to_string(),
            passed: true,
            assertions: vec![
                EvalAssertionResult {
                    assertion_type: "FileExists".to_string(),
                    passed: true,
                    message: "pass".to_string(),
                },
                EvalAssertionResult {
                    assertion_type: "FileMatches".to_string(),
                    passed: true,
                    message: "pass".to_string(),
                },
            ],
            agent_diff: None,
            reference_diff: None,
        });

        report.add_eval_result(EvalResult {
            eval_name: "eval2".to_string(),
            passed: false,
            assertions: vec![
                EvalAssertionResult {
                    assertion_type: "FileExists".to_string(),
                    passed: true,
                    message: "pass".to_string(),
                },
                EvalAssertionResult {
                    assertion_type: "FileMatches".to_string(),
                    passed: false,
                    message: "fail".to_string(),
                },
            ],
            agent_diff: None,
            reference_diff: None,
        });

        assert_eq!(report.total_evals(), 2);
        assert_eq!(report.passed_evals(), 1);
        assert_eq!(report.failed_evals(), 1);
        assert_eq!(report.total_assertions(), 4);
        assert_eq!(report.passed_assertions(), 3);
        assert_eq!(report.failed_assertions(), 1);

        // Test completion
        report.complete(Utc::now());
        assert!(report.completed_at.is_some());
    }
}

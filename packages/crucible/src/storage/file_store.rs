use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use uuid::Uuid;

use tracing_subscriber::{Layer, Registry};

use crate::storage::{
    EvalResult, Result as StoreResult, ResultsStore, RunResult, StructuredLayer, TraceEvent,
};

/// File system-based implementation of ResultsStore
#[derive(Clone)]
pub struct FileSystemStore {
    output_dir: PathBuf,
}

impl FileSystemStore {
    pub fn new(output_dir: PathBuf) -> StoreResult<Self> {
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

    fn run_result_file(&self, run_id: Uuid) -> PathBuf {
        self.run_dir(run_id).join("run_result.json")
    }

    fn parse_traces_file(
        traces_file: &Path,
    ) -> Result<HashMap<String, Vec<TraceEvent>>, Box<dyn std::error::Error>> {
        let file = fs::File::open(traces_file)?;
        let reader = BufReader::new(file);

        let mut all_traces = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            // Parse JSON and extract into our DTO
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(event_obj) = value.get("event") {
                    // Extract event data from JSON
                    let metadata = event_obj.get("metadata");
                    let fields = event_obj.get("fields");

                    let level = metadata
                        .and_then(|m| m.get("level"))
                        .and_then(|l| l.as_str())
                        .unwrap_or("UNKNOWN")
                        .to_string();

                    let target = metadata
                        .and_then(|m| m.get("target"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Extract message from fields.message.Debug
                    let message = fields
                        .and_then(|f| f.get("message"))
                        .and_then(|m| m.get("Debug"))
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string());

                    all_traces.push(TraceEvent::Event {
                        level,
                        target,
                        message,
                        fields: fields.cloned().unwrap_or_default(),
                    });
                    continue;
                }

                if let Some(span_obj) = value.get("new_span") {
                    // Extract span data from JSON
                    if let Some(attrs_obj) = span_obj.get("attributes") {
                        let metadata = attrs_obj.get("metadata");
                        let fields = attrs_obj.get("fields");

                        let name = metadata
                            .and_then(|m| m.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();

                        let level = metadata
                            .and_then(|m| m.get("level"))
                            .and_then(|l| l.as_str())
                            .unwrap_or("UNKNOWN")
                            .to_string();

                        let target = metadata
                            .and_then(|m| m.get("target"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();

                        all_traces.push(TraceEvent::Span {
                            name,
                            level,
                            target,
                            fields: fields.cloned().unwrap_or_default(),
                        });
                        continue;
                    }
                }

                // If we couldn't parse it as either, mark as Other
                all_traces.push(TraceEvent::Other);
            }
        }

        // Put all traces in ungrouped
        let mut grouped = HashMap::new();
        grouped.insert("_ungrouped".to_string(), all_traces);

        Ok(grouped)
    }
}

impl ResultsStore for FileSystemStore {
    async fn save_eval_result(
        &self,
        run_id: Uuid,
        eval_name: &str,
        report: &EvalResult,
    ) -> StoreResult<()> {
        let result_file = self.result_file(run_id, eval_name);
        // Ensure the results directory exists
        if let Some(parent) = result_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(report).map_err(|e| Box::new(e))?;
        fs::write(result_file, json).map_err(|e| Box::new(e))?;
        Ok(())
    }

    async fn save_run_result(&self, run_id: Uuid, result: &RunResult) -> StoreResult<()> {
        let result_file = self.run_result_file(run_id);
        if let Some(parent) = result_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(result).map_err(|e| Box::new(e))?;
        fs::write(result_file, json).map_err(|e| Box::new(e))?;
        Ok(())
    }

    async fn save_trace_events(
        &self,
        run_id: Uuid,
    ) -> StoreResult<HashMap<String, Vec<TraceEvent>>> {
        let traces_file = self.traces_file(run_id);
        let traces = Self::parse_traces_file(&traces_file)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { format!("{}", e).into() })?;
        Ok(traces)
    }

    async fn get_run_result(&self, run_id: Uuid) -> StoreResult<RunResult> {
        let result_file = self.run_result_file(run_id);
        let json = fs::read_to_string(&result_file).map_err(|e| Box::new(e))?;
        let result: RunResult = serde_json::from_str(&json).map_err(|e| Box::new(e))?;
        Ok(result)
    }

    async fn get_eval_result(&self, run_id: Uuid, eval_name: &str) -> StoreResult<EvalResult> {
        let result_file = self.result_file(run_id, eval_name);
        let json = fs::read_to_string(&result_file).map_err(|e| Box::new(e))?;
        let report: EvalResult = serde_json::from_str(&json).map_err(|e| Box::new(e))?;
        Ok(report)
    }

    fn create_tracing_layer(&self, run_id: Uuid) -> Box<dyn Layer<Registry> + Send + Sync> {
        let traces_file = self.traces_file(run_id);
        // Ensure the run directory exists
        if let Some(parent) = traces_file.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Open the file for writing
        let file = fs::File::create(traces_file).expect("Failed to create traces file");

        let structured_layer = StructuredLayer::new(file);
        Box::new(structured_layer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_traces_file_structured_format() {
        // Create a temporary directory and file
        let temp_dir = TempDir::new().unwrap();
        let traces_path = temp_dir.path().join("traces.jsonl");

        // Write sample structured trace data
        let mut file = fs::File::create(&traces_path).unwrap();
        writeln!(
            file,
            r#"{{"event":{{"fields":{{"message":{{"Debug":"Test message"}},"eval_name":{{"Debug":"test_eval"}}}}, "metadata":{{"level":"INFO","target":"test","name":"event","file":"test.rs","line":1,"module_path":"test","is_event":true,"is_span":false,"fields":["message","eval_name"]}}, "parent":null}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"new_span":{{"attributes":{{"metadata":{{"name":"test_span","level":"INFO","target":"test","file":"test.rs","line":2,"module_path":"test","is_event":false,"is_span":true,"fields":["eval_name"]}},"fields":{{"eval_name":{{"Debug":"test_eval2"}}}},"is_root":false,"parent":null}}, "id":{{"id":1}}}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"event":{{"fields":{{"message":{{"Debug":"Ungrouped message"}}}}, "metadata":{{"level":"WARN","target":"test","name":"event","file":"test.rs","line":3,"module_path":"test","is_event":true,"is_span":false,"fields":["message"]}}, "parent":null}}}}"#
        )
        .unwrap();
        drop(file);

        // Parse the traces
        let result = FileSystemStore::parse_traces_file(&traces_path);
        assert!(result.is_ok(), "Failed to parse traces: {:?}", result.err());

        let grouped = result.unwrap();

        // All traces should be in _ungrouped
        assert!(grouped.contains_key("_ungrouped"));
        let ungrouped = &grouped["_ungrouped"];
        assert_eq!(ungrouped.len(), 3);

        // Verify trace types and data
        match &ungrouped[0] {
            TraceEvent::Event {
                level,
                target,
                message,
                ..
            } => {
                assert_eq!(level, "INFO");
                assert_eq!(target, "test");
                assert_eq!(message.as_deref(), Some("Test message"));
            }
            _ => panic!("Expected Event"),
        }

        match &ungrouped[1] {
            TraceEvent::Span {
                name,
                level,
                target,
                ..
            } => {
                assert_eq!(name, "test_span");
                assert_eq!(level, "INFO");
                assert_eq!(target, "test");
            }
            _ => panic!("Expected Span"),
        }

        match &ungrouped[2] {
            TraceEvent::Event { level, message, .. } => {
                assert_eq!(level, "WARN");
                assert_eq!(message.as_deref(), Some("Ungrouped message"));
            }
            _ => panic!("Expected Event"),
        }
    }
}

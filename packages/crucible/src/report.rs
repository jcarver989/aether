use crate::eval::Eval;
use crate::eval_assertion::{EvalAssertion, EvalAssertionResult};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub eval_name: String,
    pub passed: bool,
    #[serde(skip)]
    pub duration: Option<Duration>,
    pub assertions: Vec<AssertionReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_diff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gold_diff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_stats: Option<DiffStats>,
}

impl EvalReport {
    /// Print the eval header
    pub fn print_header(&self) {
        println!("\n{}", format!("=== Eval: {} ===", self.eval_name).bold());
    }

    /// Write eval report to a JSON file
    pub fn write_to_file(&self, path: &Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionReport {
    pub assertion_type: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryReport {
    pub total_evals: usize,
    pub passed_evals: usize,
    pub failed_evals: usize,
    pub total_assertions: usize,
    pub passed_assertions: usize,
    pub failed_assertions: usize,
    pub evals: Vec<EvalReport>,
}

impl SummaryReport {
    pub fn new() -> Self {
        Self {
            total_evals: 0,
            passed_evals: 0,
            failed_evals: 0,
            total_assertions: 0,
            passed_assertions: 0,
            failed_assertions: 0,
            evals: Vec::new(),
        }
    }

    pub fn add_eval(&mut self, report: EvalReport) {
        self.total_evals += 1;
        if report.passed {
            self.passed_evals += 1;
        } else {
            self.failed_evals += 1;
        }

        for assertion in &report.assertions {
            self.total_assertions += 1;
            if assertion.passed {
                self.passed_assertions += 1;
            } else {
                self.failed_assertions += 1;
            }
        }

        self.evals.push(report);
    }

    /// Print the summary to stdout
    pub fn print(&self) {
        println!("\n{}", "=== Summary ===".bold());
        println!(
            "Evals: {} passed, {} failed, {} total",
            self.passed_evals.to_string().green(),
            self.failed_evals.to_string().red(),
            self.total_evals
        );
        println!(
            "Assertions: {} passed, {} failed, {} total",
            self.passed_assertions.to_string().green(),
            self.failed_assertions.to_string().red(),
            self.total_assertions
        );

        if self.failed_evals > 0 {
            println!("\n{}", "Failed evals:".red().bold());
            for eval in &self.evals {
                if !eval.passed {
                    println!("  {} {}", "✗".red(), eval.eval_name);
                }
            }
        }

        println!();
    }

    /// Write summary report to a JSON file
    pub fn write_to_file(&self, path: &Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

impl Default for SummaryReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute basic diff statistics from a git diff string
pub fn compute_diff_stats(diff: &str) -> DiffStats {
    let mut lines_added = 0;
    let mut lines_removed = 0;
    let mut files_changed = 0;

    for line in diff.lines() {
        if line.starts_with("diff --git") {
            files_changed += 1;
        } else if line.starts_with('+') && !line.starts_with("+++") {
            lines_added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            lines_removed += 1;
        }
    }

    DiffStats {
        files_changed,
        lines_added,
        lines_removed,
    }
}

pub fn create_eval_report(
    eval: &Eval,
    results: &[(EvalAssertion, EvalAssertionResult)],
    duration: Option<Duration>,
) -> EvalReport {
    let assertions: Vec<AssertionReport> = results
        .iter()
        .map(|(assertion, result)| AssertionReport {
            assertion_type: match assertion {
                EvalAssertion::FileExists { .. } => "FileExists".to_string(),
                EvalAssertion::FileMatches { .. } => "FileMatches".to_string(),
                EvalAssertion::LLMJudge { .. } => "LLMJudge".to_string(),
                EvalAssertion::CommandExitCode { .. } => "CommandExitCode".to_string(),
                EvalAssertion::ToolCall { .. } => "ToolCall".to_string(),
            },
            passed: result.is_success(),
            message: result.message().to_string(),
        })
        .collect();

    let passed = assertions.iter().all(|a| a.passed);

    EvalReport {
        eval_name: eval.name.clone(),
        passed,
        duration,
        assertions,
        agent_diff: None,
        gold_diff: None,
        diff_stats: None,
    }
}

// HTML Report Generation

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_name: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    pub summary: SummaryReport,
    pub eval_traces: HashMap<String, Vec<TraceEvent>>,
}


pub fn parse_traces_file(
    traces_file: &Path,
) -> Result<HashMap<String, Vec<TraceEvent>>, Box<dyn std::error::Error>> {
    let file = fs::File::open(traces_file)?;
    let reader = BufReader::new(file);

    let mut grouped: HashMap<String, Vec<TraceEvent>> = HashMap::new();
    grouped.insert("_ungrouped".to_string(), Vec::new());

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(&line) {
            Ok(json) => {
                let trace = TraceEvent {
                    timestamp: json
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    level: json
                        .get("level")
                        .and_then(|v| v.as_str())
                        .unwrap_or("INFO")
                        .to_string(),
                    message: json
                        .get("fields")
                        .and_then(|f| f.get("message"))
                        .and_then(|v| v.as_str())
                        .or_else(|| json.get("message").and_then(|v| v.as_str()))
                        .unwrap_or("")
                        .to_string(),
                    target: json
                        .get("target")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    eval_name: json
                        .get("span")
                        .and_then(|s| s.get("eval_name"))
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            json.get("spans")
                                .and_then(|spans| spans.as_array())
                                .and_then(|arr| {
                                    arr.iter().find_map(|span| {
                                        span.get("eval_name").and_then(|v| v.as_str())
                                    })
                                })
                        })
                        .map(|s| s.to_string()),
                    extra: json
                        .as_object()
                        .map(|obj| {
                            obj.iter()
                                .filter(|(k, _)| {
                                    !matches!(
                                        k.as_str(),
                                        "timestamp"
                                            | "level"
                                            | "message"
                                            | "target"
                                            | "span"
                                            | "spans"
                                    )
                                })
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect()
                        })
                        .unwrap_or_default(),
                };

                // Group by eval_name if present
                if let Some(eval_name) = &trace.eval_name {
                    grouped.entry(eval_name.clone()).or_default().push(trace);
                } else {
                    grouped.get_mut("_ungrouped").unwrap().push(trace);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to parse trace line: {}", e);
            }
        }
    }

    Ok(grouped)
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;


    #[test]
    fn test_parse_and_group_traces() {
        let temp_dir = TempDir::new().unwrap();
        let traces_file = temp_dir.path().join("traces.jsonl");
        let mut file = std::fs::File::create(&traces_file).unwrap();

        // Write traces for multiple evals
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-01T12:00:00Z","level":"INFO","message":"Eval 1","span":{{"eval_name":"eval_one"}}}}"#
        ).unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-01T12:00:01Z","level":"INFO","message":"Eval 2","span":{{"eval_name":"eval_two"}}}}"#
        ).unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-01T12:00:02Z","level":"INFO","message":"Ungrouped"}}"#
        )
        .unwrap();

        let grouped = parse_traces_file(&traces_file).unwrap();

        assert!(
            grouped.contains_key("eval_one"),
            "Should have eval_one group"
        );
        assert!(
            grouped.contains_key("eval_two"),
            "Should have eval_two group"
        );
        assert!(
            grouped.contains_key("_ungrouped"),
            "Should have _ungrouped group"
        );

        assert_eq!(grouped["eval_one"].len(), 1, "eval_one should have 1 trace");
        assert_eq!(grouped["eval_two"].len(), 1, "eval_two should have 1 trace");
        assert_eq!(
            grouped["_ungrouped"].len(),
            1,
            "_ungrouped should have 1 trace"
        );

        assert_eq!(grouped["eval_one"][0].message, "Eval 1");
        assert_eq!(grouped["eval_two"][0].message, "Eval 2");
    }


    #[test]
    fn test_compute_diff_stats() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,7 @@
 fn main() {
-    println!("Hello");
+    println!("Hello, world!");
+    println!("New line");
 }
+
+// New comment
diff --git a/src/lib.rs b/src/lib.rs
index 9876543..fedcba9 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,2 @@
-pub fn old_func() {}
+pub fn new_func() {}
"#;

        let stats = compute_diff_stats(diff);

        assert_eq!(stats.files_changed, 2, "Should detect 2 files changed");
        // Added lines: +    println!("Hello, world!"); +    println!("New line"); + (empty line) +// New comment +pub fn new_func() {} = 5 total
        assert_eq!(stats.lines_added, 5, "Should count 5 added lines");
        assert_eq!(stats.lines_removed, 2, "Should count 2 removed lines");
    }

    #[test]
    fn test_compute_diff_stats_empty() {
        let diff = "";
        let stats = compute_diff_stats(diff);

        assert_eq!(stats.files_changed, 0);
        assert_eq!(stats.lines_added, 0);
        assert_eq!(stats.lines_removed, 0);
    }

    #[test]
    fn test_eval_report_with_diffs_serialization() {
        let report = EvalReport {
            eval_name: "test_eval".to_string(),
            passed: true,
            duration: None,
            assertions: vec![],
            agent_diff: Some("diff --git a/file.txt b/file.txt\n+added line".to_string()),
            gold_diff: Some("diff --git a/file.txt b/file.txt\n+gold line".to_string()),
            diff_stats: Some(DiffStats {
                files_changed: 1,
                lines_added: 5,
                lines_removed: 2,
            }),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("agent_diff"));
        assert!(json.contains("gold_diff"));
        assert!(json.contains("diff_stats"));
        assert!(json.contains("files_changed"));
        assert!(json.contains("lines_added"));
        assert!(json.contains("lines_removed"));

        // Test deserialization
        let deserialized: EvalReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.eval_name, "test_eval");
        assert!(deserialized.agent_diff.is_some());
        assert!(deserialized.gold_diff.is_some());
        assert!(deserialized.diff_stats.is_some());

        let stats = deserialized.diff_stats.unwrap();
        assert_eq!(stats.files_changed, 1);
        assert_eq!(stats.lines_added, 5);
        assert_eq!(stats.lines_removed, 2);
    }

    #[test]
    fn test_eval_report_without_diffs_serialization() {
        let report = EvalReport {
            eval_name: "test_eval".to_string(),
            passed: true,
            duration: None,
            assertions: vec![],
            agent_diff: None,
            gold_diff: None,
            diff_stats: None,
        };

        // Test JSON serialization - should not include null fields due to skip_serializing_if
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("agent_diff"));
        assert!(!json.contains("gold_diff"));
        assert!(!json.contains("diff_stats"));

        // Test deserialization
        let deserialized: EvalReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.eval_name, "test_eval");
        assert!(deserialized.agent_diff.is_none());
        assert!(deserialized.gold_diff.is_none());
        assert!(deserialized.diff_stats.is_none());
    }
}

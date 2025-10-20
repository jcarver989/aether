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
pub struct EvalReport {
    pub eval_name: String,
    pub passed: bool,
    #[serde(skip)]
    pub duration: Option<Duration>,
    pub assertions: Vec<AssertionReport>,
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
    }
}

// HTML Report Generation

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceEvent {
    timestamp: String,
    level: String,
    message: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_name: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReportData {
    summary: SummaryReport,
    eval_traces: HashMap<String, Vec<TraceEvent>>,
}

/// Copy HTML report static files (HTML, CSS, JS) so users can view the report before it's complete
pub fn copy_report_templates(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let report_dir = output_dir.join("report");
    fs::create_dir_all(&report_dir)?;

    // Write HTML, CSS, and JS files
    fs::write(
        report_dir.join("index.html"),
        include_str!("../templates/index.html"),
    )?;
    fs::write(
        report_dir.join("styles.css"),
        include_str!("../templates/styles.css"),
    )?;
    fs::write(
        report_dir.join("script.js"),
        include_str!("../templates/script.js"),
    )?;

    // Create an initial empty report-data.json so the page loads without errors
    let empty_report = ReportData {
        summary: SummaryReport::new(),
        eval_traces: HashMap::new(),
    };
    let empty_json = serde_json::to_string_pretty(&empty_report)?;
    fs::write(report_dir.join("report-data.json"), empty_json)?;

    Ok(())
}

/// Update the report data JSON with current traces and summary
pub fn update_report_data(
    output_dir: &Path,
    summary: &SummaryReport,
    traces_file: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse traces and group by eval_name
    let eval_traces = parse_and_group_traces(traces_file)?;

    // Create report directory (in case it wasn't created yet)
    let report_dir = output_dir.join("report");
    fs::create_dir_all(&report_dir)?;

    // Create report data JSON
    let report_data = ReportData {
        summary: summary.clone(),
        eval_traces,
    };
    let data_json = serde_json::to_string_pretty(&report_data)?;
    fs::write(report_dir.join("report-data.json"), data_json)?;

    Ok(())
}

/// Generate complete HTML report with traces grouped by eval (convenience function)
pub fn generate_html_report(
    output_dir: &Path,
    summary: &SummaryReport,
    traces_file: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    copy_report_templates(output_dir)?;
    update_report_data(output_dir, summary, traces_file)?;
    Ok(())
}

fn parse_and_group_traces(
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
    fn test_html_report_generation() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create sample traces.jsonl
        let traces_file = temp_path.join("traces.jsonl");
        let mut file = std::fs::File::create(&traces_file).unwrap();

        // Write some sample JSON traces
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-01T12:00:00Z","level":"INFO","message":"Starting eval","target":"crucible::eval","span":{{"eval_name":"test_eval"}}}}"#
        ).unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-01T12:00:01Z","level":"INFO","message":"Agent response: Hello","target":"crucible::eval","span":{{"eval_name":"test_eval"}}}}"#
        ).unwrap();

        // Create sample summary
        let mut summary = SummaryReport::new();
        let eval_report = EvalReport {
            eval_name: "test_eval".to_string(),
            passed: true,
            duration: None,
            assertions: vec![AssertionReport {
                assertion_type: "FileExists".to_string(),
                passed: true,
                message: "File exists".to_string(),
            }],
        };
        summary.add_eval(eval_report);

        // Generate HTML report
        let result = generate_html_report(temp_path, &summary, &traces_file);
        assert!(
            result.is_ok(),
            "HTML report generation failed: {:?}",
            result.err()
        );

        // Verify report files were created
        let report_dir = temp_path.join("report");
        assert!(report_dir.exists(), "Report directory was not created");
        assert!(
            report_dir.join("index.html").exists(),
            "index.html was not created"
        );
        assert!(
            report_dir.join("styles.css").exists(),
            "styles.css was not created"
        );
        assert!(
            report_dir.join("script.js").exists(),
            "script.js was not created"
        );
        assert!(
            report_dir.join("report-data.json").exists(),
            "report-data.json was not created"
        );

        // Verify report-data.json contains expected data
        let data_content = std::fs::read_to_string(report_dir.join("report-data.json")).unwrap();
        assert!(
            data_content.contains("test_eval"),
            "report-data.json should contain eval name"
        );
        assert!(
            data_content.contains("Starting eval"),
            "report-data.json should contain trace message"
        );
    }

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

        let grouped = parse_and_group_traces(&traces_file).unwrap();

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
}

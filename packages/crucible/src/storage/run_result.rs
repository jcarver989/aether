use std::time::Duration;
use std::{collections::HashMap, path::Path};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use owo_colors::OwoColorize;

use crate::storage::{EvalResult, TraceEvent};

/// The result of running a set of evaluations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub batch_size: Option<usize>,
    pub batch_delay_ms: Option<u64>,
    pub total_evals: usize,
    pub passed_evals: usize,
    pub failed_evals: usize,
    pub total_assertions: usize,
    pub passed_assertions: usize,
    pub failed_assertions: usize,
    pub evals: Vec<EvalResult>,
    pub eval_traces: HashMap<String, Vec<TraceEvent>>,
}

impl RunResult {
    pub fn new(run_id: Uuid, batch_size: Option<usize>, batch_delay: Option<Duration>) -> Self {
        Self {
            id: run_id,
            started_at: Utc::now(),
            completed_at: None,
            batch_size,
            batch_delay_ms: batch_delay.map(|d| d.as_millis() as u64),
            total_evals: 0,
            passed_evals: 0,
            failed_evals: 0,
            total_assertions: 0,
            passed_assertions: 0,
            failed_assertions: 0,
            evals: Vec::new(),
            eval_traces: HashMap::new(),
        }
    }

    pub fn add_eval_result(&mut self, result: EvalResult) {
        self.total_evals += 1;
        if result.passed {
            self.passed_evals += 1;
        } else {
            self.failed_evals += 1;
        }

        for assertion in &result.assertions {
            self.total_assertions += 1;
            if assertion.passed {
                self.passed_assertions += 1;
            } else {
                self.failed_assertions += 1;
            }
        }

        self.evals.push(result);
    }

    pub fn complete(&mut self) {
        self.completed_at = Some(Utc::now());
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

impl Default for RunResult {
    fn default() -> Self {
        Self::new(Uuid::new_v4(), None, None)
    }
}

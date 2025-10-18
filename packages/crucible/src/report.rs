use crate::eval::Eval;
use crate::eval_assertion::{EvalAssertion, EvalAssertionResult};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
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

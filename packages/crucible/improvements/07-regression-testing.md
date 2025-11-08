# Regression Testing Support

**Priority:** 🟢 P2 - Medium
**Impact:** Medium
**Effort:** Medium
**Estimated LOC:** ~300

## Problem

Currently, there's no built-in way to:

1. **Compare agent versions**: "Did my code change break any evals?"
2. **Track performance over time**: "Is the agent getting better or worse?"
3. **Prevent regressions**: "Which evals used to pass but now fail?"
4. **Measure improvements**: "How many evals did this change fix?"

### Common Workflow That's Not Supported

```bash
# Current workflow (manual comparison)
cargo run --example evals > baseline.txt
git checkout feature-branch
cargo run --example evals > feature.txt
diff baseline.txt feature.txt  # Manual, tedious, error-prone
```

What we want:

```bash
# Desired workflow
crucible run --save-baseline
git checkout feature-branch
crucible run --compare-to-baseline
# Shows: 2 regressions, 5 improvements, 93 unchanged
```

## Solution

Add regression testing capabilities to track and compare eval runs over time.

### Data Model

```rust
// In packages/crucible/src/storage/models.rs

/// Metadata about an eval run for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetadata {
    pub run_id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub agent_version: Option<String>,
    pub total_evals: usize,
    pub passed_evals: usize,
    pub failed_evals: usize,
    pub total_cost_usd: f64,
    pub tags: HashMap<String, String>,
}

/// Comparison between two eval runs
#[derive(Debug, Serialize, Deserialize)]
pub struct RunComparison {
    pub baseline: RunMetadata,
    pub current: RunMetadata,

    /// Evals that passed in baseline but failed in current
    pub regressions: Vec<EvalComparison>,

    /// Evals that failed in baseline but passed in current
    pub improvements: Vec<EvalComparison>,

    /// Evals that changed from pass to pass or fail to fail, but assertions changed
    pub changed: Vec<EvalComparison>,

    /// Evals with same results
    pub unchanged: Vec<String>,

    /// Summary statistics
    pub summary: ComparisonSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvalComparison {
    pub eval_name: String,
    pub baseline_result: EvalOutcome,
    pub current_result: EvalOutcome,
    pub assertion_diff: Vec<AssertionDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalOutcome {
    pub passed: bool,
    pub assertions_passed: usize,
    pub assertions_failed: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssertionDiff {
    pub assertion: String,
    pub baseline: AssertionStatus,
    pub current: AssertionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssertionStatus {
    Passed,
    Failed(String), // Failure message
    NotRun,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub total_evals: usize,
    pub regressions_count: usize,
    pub improvements_count: usize,
    pub changed_count: usize,
    pub unchanged_count: usize,
    pub cost_delta_usd: f64,
}
```

### Extend ResultsStore Trait

```rust
// In packages/crucible/src/storage/store.rs

pub trait ResultsStore: Send + Sync + Clone {
    // ... existing methods ...

    /// Save metadata about a run
    fn save_run_metadata(&self, metadata: &RunMetadata) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Get metadata for a specific run
    fn get_run_metadata(&self, run_id: Uuid) -> impl Future<Output = Result<Option<RunMetadata>, StorageError>> + Send;

    /// List all runs with their metadata
    fn list_runs(&self) -> impl Future<Output = Result<Vec<RunMetadata>, StorageError>> + Send;

    /// Get the most recent run (useful for baseline comparison)
    fn get_latest_run(&self) -> impl Future<Output = Result<Option<RunMetadata>, StorageError>> + Send;

    /// Compare two runs and generate diff
    fn compare_runs(
        &self,
        baseline_id: Uuid,
        current_id: Uuid,
    ) -> impl Future<Output = Result<RunComparison, ComparisonError>> + Send;
}
```

### Implementation in FileSystemStore

```rust
// In packages/crucible/src/storage/file_store.rs

impl FileSystemStore {
    fn metadata_file(&self, run_id: Uuid) -> PathBuf {
        self.run_dir(run_id).join("metadata.json")
    }

    fn runs_index_file(&self) -> PathBuf {
        self.output_dir.join("runs.json")
    }
}

impl ResultsStore for FileSystemStore {
    async fn save_run_metadata(&self, metadata: &RunMetadata) -> Result<()> {
        let metadata_path = self.metadata_file(metadata.run_id);
        let json = serde_json::to_string_pretty(metadata)?;
        fs::write(metadata_path, json)?;

        // Update index
        self.update_runs_index(metadata).await?;

        Ok(())
    }

    async fn get_run_metadata(&self, run_id: Uuid) -> Result<Option<RunMetadata>> {
        let path = self.metadata_file(run_id);
        if !path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(path)?;
        let metadata = serde_json::from_str(&json)?;
        Ok(Some(metadata))
    }

    async fn list_runs(&self) -> Result<Vec<RunMetadata>> {
        let index_path = self.runs_index_file();
        if !index_path.exists() {
            return Ok(Vec::new());
        }

        let json = fs::read_to_string(index_path)?;
        let runs: Vec<RunMetadata> = serde_json::from_str(&json)?;
        Ok(runs)
    }

    async fn get_latest_run(&self) -> Result<Option<RunMetadata>> {
        let mut runs = self.list_runs().await?;
        runs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(runs.into_iter().next())
    }

    async fn compare_runs(
        &self,
        baseline_id: Uuid,
        current_id: Uuid,
    ) -> Result<RunComparison> {
        let baseline_meta = self.get_run_metadata(baseline_id).await?
            .ok_or("Baseline run not found")?;
        let current_meta = self.get_run_metadata(current_id).await?
            .ok_or("Current run not found")?;

        let baseline_results = self.get_eval_results(baseline_id).await;
        let current_results = self.get_eval_results(current_id).await;

        let mut regressions = Vec::new();
        let mut improvements = Vec::new();
        let mut changed = Vec::new();
        let mut unchanged = Vec::new();

        // Build maps for easy lookup
        let baseline_map: HashMap<String, &EvalResult> = baseline_results
            .iter()
            .filter_map(|r| {
                if let EvalResult::Completed { eval_name, .. } = r {
                    Some((eval_name.clone(), r))
                } else {
                    None
                }
            })
            .collect();

        let current_map: HashMap<String, &EvalResult> = current_results
            .iter()
            .filter_map(|r| {
                if let EvalResult::Completed { eval_name, .. } = r {
                    Some((eval_name.clone(), r))
                } else {
                    None
                }
            })
            .collect();

        // Compare results
        for (eval_name, baseline_result) in &baseline_map {
            if let Some(current_result) = current_map.get(eval_name) {
                let comparison = Self::compare_eval_results(
                    eval_name,
                    baseline_result,
                    current_result,
                );

                match (&comparison.baseline_result.passed, &comparison.current_result.passed) {
                    (true, false) => regressions.push(comparison),
                    (false, true) => improvements.push(comparison),
                    (true, true) | (false, false) => {
                        if !comparison.assertion_diff.is_empty() {
                            changed.push(comparison);
                        } else {
                            unchanged.push(eval_name.clone());
                        }
                    }
                }
            }
        }

        let summary = ComparisonSummary {
            total_evals: baseline_map.len().max(current_map.len()),
            regressions_count: regressions.len(),
            improvements_count: improvements.len(),
            changed_count: changed.len(),
            unchanged_count: unchanged.len(),
            cost_delta_usd: current_meta.total_cost_usd - baseline_meta.total_cost_usd,
        };

        Ok(RunComparison {
            baseline: baseline_meta,
            current: current_meta,
            regressions,
            improvements,
            changed,
            unchanged,
            summary,
        })
    }
}
```

### Public API

```rust
// In packages/crucible/src/lib.rs

impl EvalRunner<R, T> {
    /// Run evals and save metadata for future comparison
    pub async fn run_with_metadata<J>(
        self,
        evals: Vec<Eval>,
        config: EvalsConfig<J>,
        tags: HashMap<String, String>,
    ) -> Result<(Uuid, RunSummary), Box<dyn std::error::Error>>
    where
        J: StreamingModelProvider + 'static,
    {
        let run_id = self.run_evals(evals, config).await?;

        // Collect git info
        let git_commit = Self::get_git_commit().ok();
        let git_branch = Self::get_git_branch().ok();

        let results = self.results_store.get_eval_results(run_id).await;
        let passed = results.iter().filter(|r| r.passed()).count();
        let failed = results.len() - passed;

        let total_cost = results
            .iter()
            .filter_map(|r| r.cost_metrics())
            .map(|c| c.estimated_cost_usd)
            .sum();

        let metadata = RunMetadata {
            run_id,
            timestamp: chrono::Utc::now(),
            git_commit,
            git_branch,
            agent_version: None,
            total_evals: results.len(),
            passed_evals: passed,
            failed_evals: failed,
            total_cost_usd: total_cost,
            tags,
        };

        self.results_store.save_run_metadata(&metadata).await?;

        Ok((run_id, RunSummary { /* ... */ }))
    }

    /// Compare current run against a baseline
    pub async fn compare_to_baseline(
        &self,
        current_run_id: Uuid,
        baseline_run_id: Uuid,
    ) -> Result<RunComparison, Box<dyn std::error::Error>> {
        self.results_store.compare_runs(baseline_run_id, current_run_id).await
    }
}
```

## Usage Examples

### Save Baseline

```rust
let runner = EvalRunner::new(agent_runner, store);

let mut tags = HashMap::new();
tags.insert("baseline".to_string(), "v1.0".to_string());

let (baseline_id, _) = runner
    .run_with_metadata(evals, config, tags)
    .await?;

println!("Baseline run: {}", baseline_id);
```

### Compare Against Baseline

```rust
// Make code changes...

let (current_id, _) = runner
    .run_with_metadata(evals, config, HashMap::new())
    .await?;

let comparison = runner.compare_to_baseline(current_id, baseline_id).await?;

println!("Regressions: {}", comparison.summary.regressions_count);
println!("Improvements: {}", comparison.summary.improvements_count);

if comparison.summary.regressions_count > 0 {
    eprintln!("❌ REGRESSIONS DETECTED:");
    for regression in &comparison.regressions {
        eprintln!("  - {}", regression.eval_name);
    }
    std::process::exit(1);
}
```

### Generate HTML Report

```rust
// In packages/crucible/src/reporting.rs

pub fn generate_comparison_report(
    comparison: &RunComparison,
    output_path: &Path,
) -> Result<()> {
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head><title>Eval Comparison</title></head>
        <body>
            <h1>Eval Run Comparison</h1>
            <h2>Summary</h2>
            <ul>
                <li>Regressions: {}</li>
                <li>Improvements: {}</li>
                <li>Changed: {}</li>
                <li>Unchanged: {}</li>
            </ul>
            <!-- ... detailed tables ... -->
        </body>
        </html>
        "#,
        comparison.summary.regressions_count,
        comparison.summary.improvements_count,
        comparison.summary.changed_count,
        comparison.summary.unchanged_count,
    );

    fs::write(output_path, html)?;
    Ok(())
}
```

## Error Handling

Following the error-handling best practice (no `anyhow` or `Box<dyn Error>`), define specific error types:

```rust
// In packages/crucible/src/storage/error.rs

#[derive(Debug)]
pub enum StorageError {
    FileNotFound { path: String },
    IoError { path: String, error: String },
    SerializationError { reason: String },
    InvalidData { reason: String },
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileNotFound { path } => write!(f, "File not found: {}", path),
            Self::IoError { path, error } => write!(f, "IO error at {}: {}", path, error),
            Self::SerializationError { reason } => write!(f, "Serialization error: {}", reason),
            Self::InvalidData { reason } => write!(f, "Invalid data: {}", reason),
        }
    }
}

impl std::error::Error for StorageError {}

#[derive(Debug)]
pub enum ComparisonError {
    BaselineNotFound { run_id: Uuid },
    CurrentNotFound { run_id: Uuid },
    IncompatibleRuns { reason: String },
    StorageError(StorageError),
}

impl std::fmt::Display for ComparisonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BaselineNotFound { run_id } => write!(f, "Baseline run not found: {}", run_id),
            Self::CurrentNotFound { run_id } => write!(f, "Current run not found: {}", run_id),
            Self::IncompatibleRuns { reason } => write!(f, "Incompatible runs: {}", reason),
            Self::StorageError(e) => write!(f, "Storage error: {}", e),
        }
    }
}

impl std::error::Error for ComparisonError {}

impl From<StorageError> for ComparisonError {
    fn from(e: StorageError) -> Self {
        Self::StorageError(e)
    }
}
```

## Files to Change

1. `packages/crucible/src/storage/models.rs` - Add comparison types
2. `packages/crucible/src/storage/error.rs` - NEW: Add `StorageError` and `ComparisonError` enums
3. `packages/crucible/src/storage/store.rs` - Extend trait with comparison methods
4. `packages/crucible/src/storage/file_store.rs` - Implement comparison logic
5. `packages/crucible/src/evals/runner.rs` - Add `run_with_metadata()` method
6. `packages/crucible/src/reporting.rs` - NEW: HTML report generation
7. `packages/crucible/Cargo.toml` - Add `chrono` dependency

## Benefits

1. **Prevent Regressions**: Catch when code changes break evals
2. **Track Progress**: Measure improvements over time
3. **CI/CD Integration**: Fail builds on regressions
4. **Performance Insights**: Track cost and speed trends
5. **A/B Testing**: Compare different agent configurations

## Testing Strategy

Following the testing-fakes.md best practice (use "Fake", never "Mock"):

1. Create a `FakeResultsStore` with in-memory storage for run metadata
2. Run same evals twice, verify unchanged count = total using fake store
3. Use `FakeAgentRunner` to simulate regressions and improvements
4. Test git metadata extraction with deterministic fake git commands
5. Verify HTML report generation with known comparison data
6. Test error handling by pattern matching on specific error types

Example test:
```rust
#[tokio::test]
async fn test_detect_regression() {
    let fake_store = FakeResultsStore::new();

    // Run 1: All pass
    let fake_runner = FakeAgentRunner::with_success();
    let baseline_id = run_evals_and_save(&fake_runner, &fake_store).await;

    // Run 2: One fails
    let fake_runner = FakeAgentRunner::with_failure_at(2);
    let current_id = run_evals_and_save(&fake_runner, &fake_store).await;

    // Compare
    let comparison = fake_store.compare_runs(baseline_id, current_id).await.unwrap();

    assert_eq!(comparison.summary.regressions_count, 1);
    assert_eq!(comparison.regressions[0].eval_name, "eval_2");
}

#[tokio::test]
async fn test_missing_baseline_error() {
    let fake_store = FakeResultsStore::new();
    let missing_id = Uuid::new_v4();
    let current_id = Uuid::new_v4();

    let result = fake_store.compare_runs(missing_id, current_id).await;

    // Pattern match on specific error type
    match result {
        Err(ComparisonError::BaselineNotFound { run_id }) => {
            assert_eq!(run_id, missing_id);
        }
        _ => panic!("Expected BaselineNotFound error"),
    }
}
```

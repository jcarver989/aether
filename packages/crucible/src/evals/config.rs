use std::time::Duration;

pub struct EvalsConfig<J> {
    pub(crate) judge_llm: J,
    pub(crate) batch_size: Option<usize>,
    pub(crate) batch_delay: Option<Duration>,
    pub(crate) serve: bool,
}

impl<J> EvalsConfig<J> {
    pub fn new(judge_llm: J) -> Self {
        Self {
            judge_llm,
            batch_size: None,
            batch_delay: None,
            serve: false,
        }
    }

    /// Set the batch size for concurrent evaluation execution
    ///
    /// When running many evaluations, you may want to limit the number of concurrent
    /// evaluations to avoid rate limiting with LLM providers. This setting controls
    /// how many evals are run concurrently in each batch.
    ///
    /// # Arguments
    /// * `batch_size` - Number of evals to run concurrently in each batch
    ///
    /// # Example
    /// ```no_run
    /// use crucible::EvalsConfig;
    /// use std::time::Duration;
    /// // let judge_llm = ...; // Your judge LLM provider
    /// // let config = EvalsConfig::new(judge_llm)
    /// //     .with_batch_size(3)  // Run 3 evals at a time
    /// //     .with_batch_delay(Duration::from_secs(2));  // Wait 2 seconds between batches
    /// ```
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = Some(batch_size);
        self
    }

    /// Set the delay between batches
    ///
    /// This adds a delay between processing batches to further prevent rate limiting.
    /// The delay is only applied between batches, not within a batch.
    ///
    /// # Arguments
    /// * `delay` - Delay between batches to prevent rate limiting
    ///
    /// # Example
    /// ```
    /// use std::time::Duration;
    /// let delay = Duration::from_millis(2000);
    /// ```
    pub fn with_batch_delay(mut self, delay: Duration) -> Self {
        self.batch_delay = Some(delay);
        self
    }

    /// Enable serving the HTML report on localhost:3000 after evals complete
    ///
    /// When enabled, after all evaluations finish, a web server will start on
    /// localhost:3000 to serve the interactive HTML report. The server will run
    /// until interrupted with Ctrl+C.
    ///
    /// # Example
    /// ```no_run
    /// use crucible::EvalsConfig;
    /// // let judge_llm = ...; // Your judge LLM provider
    /// // let config = EvalsConfig::new(judge_llm)
    /// //     .with_serve(true);  // Start web server after evals
    /// ```
    pub fn with_serve(mut self, serve: bool) -> Self {
        self.serve = serve;
        self
    }
}

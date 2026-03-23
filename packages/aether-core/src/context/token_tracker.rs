/// Default threshold for triggering context compaction (85%)
pub const DEFAULT_COMPACTION_THRESHOLD: f64 = 0.85;

/// Tracks token usage from LLM API responses.
/// Uses real usage data from API, not estimation.
#[derive(Debug, Clone, Default)]
pub struct TokenTracker {
    /// Total input tokens across all API calls
    total_input_tokens: u64,
    /// Total output tokens across all API calls
    total_output_tokens: u64,
    /// Total cached input tokens across all API calls
    total_cached_input_tokens: u64,
    /// Input tokens from the most recent API call (current context size)
    last_input_tokens: u32,
    /// Cached input tokens from the most recent API call
    last_cached_input_tokens: Option<u32>,
    /// Configured context limit for the current provider
    context_limit: Option<u32>,
}

impl TokenTracker {
    pub fn new(context_limit: Option<u32>) -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cached_input_tokens: 0,
            last_input_tokens: 0,
            last_cached_input_tokens: None,
            context_limit,
        }
    }

    /// Record usage from an LLM API response
    pub fn record_usage(
        &mut self,
        input_tokens: u32,
        output_tokens: u32,
        cached_input_tokens: Option<u32>,
    ) {
        self.total_input_tokens += u64::from(input_tokens);
        self.total_output_tokens += u64::from(output_tokens);
        if let Some(cached) = cached_input_tokens {
            self.total_cached_input_tokens += u64::from(cached);
        }
        self.last_input_tokens = input_tokens;
        self.last_cached_input_tokens = cached_input_tokens;
    }

    /// Current context usage as a ratio (0.0 - 1.0)
    pub fn usage_ratio(&self) -> Option<f64> {
        let context_limit = self.context_limit?;
        if context_limit == 0 {
            return None;
        }
        Some(f64::from(self.last_input_tokens) / f64::from(context_limit))
    }

    /// Whether current usage exceeds the given threshold
    pub fn exceeds_threshold(&self, threshold: f64) -> bool {
        self.usage_ratio().is_some_and(|ratio| ratio >= threshold)
    }

    /// Check if context should be compacted based on the given threshold.
    /// This is a convenience method that combines usage ratio check with
    /// a minimum context size requirement to avoid unnecessary compaction
    /// on small conversations.
    pub fn should_compact(&self, threshold: f64) -> bool {
        let Some(context_limit) = self.context_limit else {
            return false;
        };
        let min_tokens = std::cmp::max(context_limit / 10, 1000);
        self.last_input_tokens >= min_tokens && self.exceeds_threshold(threshold)
    }

    /// Tokens remaining before hitting limit
    pub fn tokens_remaining(&self) -> Option<u32> {
        self.context_limit
            .map(|context_limit| context_limit.saturating_sub(self.last_input_tokens))
    }

    /// Update the context limit (e.g. when switching models)
    pub fn set_context_limit(&mut self, limit: Option<u32>) {
        self.context_limit = limit;
    }

    /// Get the context limit
    pub fn context_limit(&self) -> Option<u32> {
        self.context_limit
    }

    /// Get last recorded input tokens (current context size)
    pub fn last_input_tokens(&self) -> u32 {
        self.last_input_tokens
    }

    /// Get total input tokens across all calls
    pub fn total_input_tokens(&self) -> u64 {
        self.total_input_tokens
    }

    /// Get total output tokens across all calls
    pub fn total_output_tokens(&self) -> u64 {
        self.total_output_tokens
    }

    /// Get total cached input tokens across all calls
    pub fn total_cached_input_tokens(&self) -> u64 {
        self.total_cached_input_tokens
    }

    /// Get last recorded cached input tokens
    pub fn last_cached_input_tokens(&self) -> Option<u32> {
        self.last_cached_input_tokens
    }

    /// Reset current usage tracking after context compaction.
    /// Preserves cumulative totals for metrics while clearing the
    /// `last_input_tokens` to prevent immediate re-triggering of compaction.
    pub fn reset_current_usage(&mut self) {
        self.last_input_tokens = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_tracking() {
        let mut tracker = TokenTracker::new(Some(1000));

        tracker.record_usage(500, 100, None);
        assert_eq!(tracker.usage_ratio(), Some(0.5));
        assert!(!tracker.exceeds_threshold(0.85));

        tracker.record_usage(900, 50, None);
        assert_eq!(tracker.usage_ratio(), Some(0.9));
        assert!(tracker.exceeds_threshold(0.85));
    }

    #[test]
    fn test_tokens_remaining() {
        let mut tracker = TokenTracker::new(Some(1000));
        tracker.record_usage(700, 50, None);
        assert_eq!(tracker.tokens_remaining(), Some(300));
    }

    #[test]
    fn test_cumulative_totals() {
        let mut tracker = TokenTracker::new(Some(1000));
        tracker.record_usage(100, 50, None);
        tracker.record_usage(200, 60, None);

        assert_eq!(tracker.total_input_tokens(), 300);
        assert_eq!(tracker.total_output_tokens(), 110);
        assert_eq!(tracker.last_input_tokens(), 200); // Only last call
    }

    #[test]
    fn test_unknown_context_limit() {
        let tracker = TokenTracker::new(None);
        assert_eq!(tracker.usage_ratio(), None);
        assert_eq!(tracker.tokens_remaining(), None);
        assert!(!tracker.should_compact(0.85));
    }

    #[test]
    fn test_exceeds_threshold() {
        let mut tracker = TokenTracker::new(Some(1000));

        tracker.record_usage(500, 100, None);
        assert!(!tracker.exceeds_threshold(0.6));
        assert!(tracker.exceeds_threshold(0.5));

        tracker.record_usage(850, 50, None);
        assert!(tracker.exceeds_threshold(0.8));
        assert!(tracker.exceeds_threshold(0.85));
    }

    #[test]
    fn test_should_compact() {
        let mut tracker = TokenTracker::new(Some(10000));

        tracker.record_usage(500, 100, None);
        assert!(!tracker.should_compact(0.04));

        tracker.record_usage(9000, 100, None);
        assert!(tracker.should_compact(0.85));

        tracker.record_usage(7000, 100, None);
        assert!(!tracker.should_compact(0.85));
    }

    #[test]
    fn test_default_compaction_threshold() {
        use super::DEFAULT_COMPACTION_THRESHOLD;
        assert!((DEFAULT_COMPACTION_THRESHOLD - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_set_context_limit() {
        let mut tracker = TokenTracker::new(Some(200_000));
        assert_eq!(tracker.context_limit(), Some(200_000));

        tracker.set_context_limit(Some(128_000));
        assert_eq!(tracker.context_limit(), Some(128_000));

        // Verify usage ratio recalculates against new limit
        tracker.record_usage(100_000, 50, None);
        let expected_ratio = 100_000.0 / 128_000.0;
        assert!((tracker.usage_ratio().unwrap_or_default() - expected_ratio).abs() < 0.001);
    }

    #[test]
    fn test_reset_current_usage() {
        let mut tracker = TokenTracker::new(Some(10000));
        tracker.record_usage(9000, 100, None);

        assert!(tracker.should_compact(0.85));

        tracker.reset_current_usage();

        assert_eq!(tracker.last_input_tokens(), 0);
        assert!(!tracker.should_compact(0.85));
        assert_eq!(tracker.total_input_tokens(), 9000);
        assert_eq!(tracker.total_output_tokens(), 100);
    }

    #[test]
    fn test_cached_token_tracking() {
        let mut tracker = TokenTracker::new(Some(1000));

        tracker.record_usage(500, 100, Some(200));
        assert_eq!(tracker.last_cached_input_tokens(), Some(200));
        assert_eq!(tracker.total_cached_input_tokens(), 200);

        tracker.record_usage(600, 50, Some(400));
        assert_eq!(tracker.last_cached_input_tokens(), Some(400));
        assert_eq!(tracker.total_cached_input_tokens(), 600);

        tracker.record_usage(300, 30, None);
        assert_eq!(tracker.last_cached_input_tokens(), None);
        assert_eq!(tracker.total_cached_input_tokens(), 600);
    }
}

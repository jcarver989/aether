use agent_client_protocol::{self as acp};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

type PlanEntryKey = String;

#[derive(Default)]
pub struct PlanTracker {
    entries: Vec<acp::PlanEntry>,
    completed_at: HashMap<PlanEntryKey, Instant>,
}

impl PlanTracker {
    pub fn replace(&mut self, entries: Vec<acp::PlanEntry>, now: Instant) {
        let active_keys: HashSet<_> = entries.iter().map(Self::entry_key).collect();
        self.completed_at.retain(|key, _| active_keys.contains(key));

        for entry in &entries {
            let key = Self::entry_key(entry);
            match entry.status {
                acp::PlanEntryStatus::Completed => {
                    self.completed_at.entry(key).or_insert(now);
                }
                _ => {
                    self.completed_at.remove(&key);
                }
            }
        }

        self.entries = entries;
    }

    pub fn visible_entries(&self, now: Instant, grace_period: Duration) -> Vec<acp::PlanEntry> {
        self.entries
            .iter()
            .filter(|entry| self.is_visible(entry, now, grace_period))
            .cloned()
            .collect()
    }

    pub fn needs_tick(&self, now: Instant, grace_period: Duration) -> bool {
        self.entries.iter().any(|entry| {
            entry.status == acp::PlanEntryStatus::InProgress
                || matches!(entry.status, acp::PlanEntryStatus::Completed)
                    && self.is_visible(entry, now, grace_period)
        })
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.completed_at.clear();
    }

    fn is_visible(&self, entry: &acp::PlanEntry, now: Instant, grace_period: Duration) -> bool {
        match entry.status {
            acp::PlanEntryStatus::Completed => self
                .completed_at
                .get(&Self::entry_key(entry))
                .is_some_and(|completed_at| now.duration_since(*completed_at) <= grace_period),
            _ => true,
        }
    }

    /// Content is the best stable identity ACP currently gives us for plan entries.
    fn entry_key(entry: &acp::PlanEntry) -> PlanEntryKey {
        entry.content.clone()
    }

    #[cfg(test)]
    fn completed_at_for(&self, entry: &acp::PlanEntry) -> Option<Instant> {
        self.completed_at.get(&Self::entry_key(entry)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{PlanEntryPriority, PlanEntryStatus};

    const GRACE_PERIOD: Duration = Duration::from_secs(3);

    fn plan_entry(content: &str, status: PlanEntryStatus) -> acp::PlanEntry {
        acp::PlanEntry::new(content.to_string(), PlanEntryPriority::Medium, status)
    }

    #[test]
    fn completed_entry_visible_immediately_after_transition() {
        let mut tracker = PlanTracker::default();
        let now = Instant::now();

        tracker.replace(vec![plan_entry("Task A", PlanEntryStatus::Pending)], now);
        tracker.replace(vec![plan_entry("Task A", PlanEntryStatus::Completed)], now);

        let visible = tracker.visible_entries(now, GRACE_PERIOD);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].content, "Task A");
    }

    #[test]
    fn completed_entry_hidden_after_grace_period() {
        let mut tracker = PlanTracker::default();
        let now = Instant::now();

        tracker.replace(vec![plan_entry("Task A", PlanEntryStatus::Completed)], now);

        let visible =
            tracker.visible_entries(now + GRACE_PERIOD + Duration::from_millis(1), GRACE_PERIOD);
        assert!(visible.is_empty());
    }

    #[test]
    fn pending_and_in_progress_entries_remain_visible() {
        let mut tracker = PlanTracker::default();
        let now = Instant::now();

        tracker.replace(
            vec![
                plan_entry("Pending", PlanEntryStatus::Pending),
                plan_entry("Working", PlanEntryStatus::InProgress),
            ],
            now,
        );

        let visible =
            tracker.visible_entries(now + GRACE_PERIOD + Duration::from_secs(10), GRACE_PERIOD);
        let contents: Vec<_> = visible.iter().map(|entry| entry.content.as_str()).collect();
        assert_eq!(contents, vec!["Pending", "Working"]);
    }

    #[test]
    fn completion_timestamp_preserved_across_plan_updates() {
        let mut tracker = PlanTracker::default();
        let now = Instant::now();
        let entry = plan_entry("Task A", PlanEntryStatus::Completed);

        tracker.replace(vec![entry.clone()], now);
        let initial_ts = tracker
            .completed_at_for(&entry)
            .expect("timestamp should exist");

        tracker.replace(vec![entry.clone()], now + Duration::from_secs(1));
        let ts_after = tracker
            .completed_at_for(&entry)
            .expect("timestamp should exist");

        assert_eq!(initial_ts, ts_after);
    }

    #[test]
    fn completion_timestamp_cleared_when_item_becomes_non_completed() {
        let mut tracker = PlanTracker::default();
        let now = Instant::now();
        let completed = plan_entry("Task A", PlanEntryStatus::Completed);
        let pending = plan_entry("Task A", PlanEntryStatus::Pending);

        tracker.replace(vec![completed.clone()], now);
        assert!(tracker.completed_at_for(&completed).is_some());

        tracker.replace(vec![pending], now + Duration::from_secs(1));
        assert!(tracker.completed_at_for(&completed).is_none());
    }

    #[test]
    fn stale_timestamp_removed_when_item_disappears() {
        let mut tracker = PlanTracker::default();
        let now = Instant::now();
        let entry = plan_entry("Task A", PlanEntryStatus::Completed);

        tracker.replace(vec![entry.clone()], now);
        assert!(tracker.completed_at_for(&entry).is_some());

        tracker.replace(vec![], now + Duration::from_secs(1));
        assert!(tracker.completed_at_for(&entry).is_none());
    }

    #[test]
    fn mixed_entries_visible_correctly() {
        let mut tracker = PlanTracker::default();
        let now = Instant::now();

        tracker.replace(
            vec![
                plan_entry("Completed Old", PlanEntryStatus::Completed),
                plan_entry("Completed New", PlanEntryStatus::Completed),
                plan_entry("In Progress", PlanEntryStatus::InProgress),
                plan_entry("Pending", PlanEntryStatus::Pending),
            ],
            now,
        );

        tracker.replace(
            vec![
                plan_entry("Completed Old", PlanEntryStatus::Completed),
                plan_entry("Completed New", PlanEntryStatus::Completed),
                plan_entry("In Progress", PlanEntryStatus::InProgress),
                plan_entry("Pending", PlanEntryStatus::Pending),
            ],
            now + GRACE_PERIOD + Duration::from_millis(1),
        );

        let visible =
            tracker.visible_entries(now + GRACE_PERIOD + Duration::from_millis(1), GRACE_PERIOD);
        let visible_contents: Vec<_> = visible.iter().map(|e| e.content.as_str()).collect();
        assert_eq!(visible_contents, vec!["In Progress", "Pending"]);
    }
}

//! Merge strategies for combining memory stores.

/// Strategy for resolving key collisions when merging two memory stores.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MergeStrategy {
    /// On key collision, prefer the incoming (other) value. Matches legacy `mem import` behaviour.
    #[default]
    LatestWins,
    /// On key collision, keep the existing store value.
    Union,
    /// On key collision, return an error. No entries are written.
    Reject,
}

/// The outcome of a merge operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeOutcome {
    /// Number of keys inserted (did not exist in the store).
    pub inserted: usize,
    /// Number of keys updated (existed and were overwritten).
    pub updated: usize,
    /// Keys that conflicted (populated when `Reject` is used as a dry-run).
    pub conflicts: Vec<String>,
    /// Number of keys that were not changed (collision with `Union` strategy).
    pub unchanged: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latest_wins_is_default() {
        let s = MergeStrategy::default();
        assert_eq!(s, MergeStrategy::LatestWins);
    }

    #[test]
    fn test_merge_outcome_default() {
        let o = MergeOutcome::default();
        assert_eq!(o.inserted, 0);
        assert_eq!(o.updated, 0);
        assert!(o.conflicts.is_empty());
        assert_eq!(o.unchanged, 0);
    }
}
